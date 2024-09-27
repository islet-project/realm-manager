use std::collections::HashMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use ir_client::client::{Client, ImageInfo};
use ir_client::config::Config;
use ir_client::oci::reference::Reference;
use log::{debug, error, info, trace, warn};
use nix::errno::Errno;
use nix::unistd::{getgid, getuid, Gid, Group, Uid, User};
use oci_spec::image::Config as RuntimeConfig;
use oci_spec::image::ImageConfiguration as OciConfig;
use ratls::{
    load_root_cert_store, InternalTokenResolver, RaTlsCertResolver, RaTlsError, TokenFromFile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};
use thiserror::Error;

use super::handler::{ExecConfig, SimpleApplicationHandler};
use super::{ApplicationHandler, ApplicationMetadata, Launcher, Result};
use crate::config::{OciLauncherConfig, TokenResolver};
use crate::consts::{ANNOTATION_SIGNATURE, ANNOTATION_VENDORPUB, ANNOTATION_VENDORPUB_SIGNATURE};
use crate::error::Error;
use crate::util::crypto::EcdsaKey;
use crate::util::fs::{mkdirp, read_to_string, rename, rmrf, write_to_file};
use crate::util::serde::{json_dump, json_load, json_load_bytes};
use crate::util::token::RsiTokenResolver;

const METADATA: &str = "metadata.json";

#[derive(Debug, Error)]
pub enum OciLauncherError {
    #[error("Failed to load root ca")]
    RootCaLoading(#[source] RaTlsError),

    #[error("Failed to read attestation token from file")]
    TokenReadingError(#[source] RaTlsError),

    #[error("Failed to create RATLS client certificate resolver")]
    RaTlsCertResolverCreation(#[source] RaTlsError),

    #[error("Failed to create OCI client")]
    OciClientCreation(#[source] ir_client::error::Error),

    #[error("Failed to fetch image info from image registry")]
    ImageInfoFetching(#[source] ir_client::error::Error),

    #[error("Version is invalid: {1}, error: {0}")]
    InvalidVersion(#[source] ir_client::error::Error, String),

    #[error("Failed to download and unpack image")]
    UnpackError(#[source] ir_client::error::Error),

    #[error("Required image annotations weren't found: {0}")]
    FailedToFindRequiredAnnotations(String),

    #[error("Failed to hex decode annotation {0}")]
    FailedToHexDecodeAnnotation(#[source] const_hex::FromHexError, String),

    #[error("Application image has no annotations")]
    AppImageLacksAnnotations(),

    #[error("Metadata not found")]
    MetadataNotFound(),

    #[error("Runtime config is missing in image config")]
    RuntimeConfigMissing(),

    #[error("Entrypoint and cmd are missing")]
    EntryPointCmdMissing(),

    #[error("Group or User name {0} is missing")]
    InvalidUserOrGroup(String),

    #[error("Failed to resolve user name or group name")]
    GetPwName(#[source] Errno),

    #[error("Path convertsion error, {1:?} is not a valid Path")]
    PathConversionError(#[source] Infallible, String),
}

pub struct OciLauncher {
    launcher_config: OciLauncherConfig,
    ca_pub: EcdsaKey
}

#[derive(Debug)]
struct RequiredAnnotations {
    pub signature: Vec<u8>,
    pub vendor_pubkey: Vec<u8>,
    pub vendor_pubkey_signature: Vec<u8>
}

fn parse_annotation(annotations: &HashMap<String, String>, id: &str) -> Result<Vec<u8>> {
    Ok(annotations
        .get(id)
        .map(const_hex::decode)
        .ok_or(OciLauncherError::FailedToFindRequiredAnnotations(ANNOTATION_SIGNATURE.to_string()))?
        .map_err(|e| OciLauncherError::FailedToHexDecodeAnnotation(e, ANNOTATION_SIGNATURE.to_string()))?)
}

impl TryFrom<&HashMap<String, String>> for RequiredAnnotations {
    type Error = Error;

    fn try_from(annotations: &HashMap<String, String>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            signature: parse_annotation(annotations, ANNOTATION_SIGNATURE)?,
            vendor_pubkey: parse_annotation(annotations, ANNOTATION_VENDORPUB)?,
            vendor_pubkey_signature: parse_annotation(annotations, ANNOTATION_VENDORPUB_SIGNATURE)?
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    vendor_cert: Vec<Vec<u8>>,
    config_hash: Vec<u8>,
    image_config: OciConfig,
}

impl OciLauncher {
    pub fn from_oci_config(launcher_config: OciLauncherConfig, ca_pub: EcdsaKey) -> Self {
        Self { launcher_config, ca_pub }
    }

    fn verify_signature(&self, annotations: &RequiredAnnotations, config: &[u8]) -> Result<()> {
        debug!("Verifying vendor public key: {:?}", annotations.vendor_pubkey);
        self.ca_pub.verify(&annotations.vendor_pubkey, &annotations.vendor_pubkey_signature)?;

        trace!("Importing vendor public key");
        let vendor_key = EcdsaKey::import(&annotations.vendor_pubkey)?;

        debug!("Verifying application signature {:?}", annotations.signature);
        vendor_key.verify(config, &annotations.signature)?;

        Ok(())
    }

    fn create_oci_client(&self, im_url: &str) -> Result<Client> {
        let config = Config::builder().host(im_url.to_owned());

        let oci_client = match &self.launcher_config {
            OciLauncherConfig::NoTLS => Client::from_config(config.no_tls()),

            OciLauncherConfig::RusTLS { root_ca } => Client::from_config(config.rustls_no_auth(
                load_root_cert_store(root_ca).map_err(OciLauncherError::RootCaLoading)?,
            )),

            OciLauncherConfig::RaTLS {
                root_ca,
                token_resolver,
            } => {
                let token_resolver: Arc<dyn InternalTokenResolver + Send + Sync> = match token_resolver {
                    TokenResolver::File(path) => Arc::new(
                        TokenFromFile::from_path(path)
                            .map_err(OciLauncherError::TokenReadingError)?,
                    ),
                    TokenResolver::Rsi => Arc::new(RsiTokenResolver::new())
                };

                let cert_resolver = RaTlsCertResolver::from_token_resolver(token_resolver)
                    .map_err(OciLauncherError::RaTlsCertResolverCreation)?;

                Client::from_config(config.ratls(
                    load_root_cert_store(root_ca).map_err(OciLauncherError::RootCaLoading)?,
                    Arc::new(cert_resolver),
                ))
            }
        };

        Ok(oci_client.map_err(OciLauncherError::OciClientCreation)?)
    }

    async fn try_read_metadata(&self, path: &Path) -> Option<Metadata> {
        read_to_string(path.join(METADATA))
            .await
            .map(json_load)
            .ok()
            .transpose()
            .ok()
            .flatten()
    }

    async fn save_metadata(&self, path: &Path, metadata: Metadata) -> Result<()> {
        write_to_file(path.join(METADATA), json_dump(metadata)?).await?;
        Ok(())
    }

    async fn move_unpack_to_img(&self, unpack: &Path, img: &Path) -> Result<()> {
        let _ = rmrf(img)
            .await
            .map_err(|e| trace!("Failed to remove {:?}, error: {:?}", img, e));
        rename(unpack, img).await?;

        Ok(())
    }
}

fn parse_argv(config: &RuntimeConfig) -> Result<Vec<String>> {
    let entrypoint = config.entrypoint();
    let cmd = config.cmd();

    let argv: Vec<String> = [entrypoint, cmd]
        .into_iter()
        .filter_map(|i| i.clone())
        .flatten()
        .collect();

    if argv.is_empty() {
        return Err(OciLauncherError::EntryPointCmdMissing().into());
    }

    Ok(argv)
}

fn parse_env(config: &RuntimeConfig) -> HashMap<String, String> {
    let env = config.env().as_ref().map(|i| {
        i.iter()
            .map(|i| {
                i.split_once('=')
                    .map(|(x, y)| (x.to_owned(), y.to_owned()))
                    .unwrap_or((i.clone(), "".to_string()))
            })
            .collect()
    });

    env.unwrap_or(std::env::vars().collect())
}

fn uid_from_str(u: &str) -> Result<Uid> {
    Ok(User::from_name(u)
        .map_err(OciLauncherError::GetPwName)?
        .ok_or(OciLauncherError::InvalidUserOrGroup(u.to_owned()))?
        .uid)
}

fn gid_from_str(g: &str) -> Result<Gid> {
    Ok(Group::from_name(g)
        .map_err(OciLauncherError::GetPwName)?
        .ok_or(OciLauncherError::InvalidUserOrGroup(g.to_owned()))?
        .gid)
}

fn parse_uid_gid(config: &RuntimeConfig) -> Result<(Uid, Gid)> {
    let user = config.user().as_ref();
    let current_uid = getuid();
    let current_gid = getgid();

    match user.map(|i| {
        i.split_once(':')
            .map_or((i.as_str(), None), |(u, g)| (u, Some(g)))
    }) {
        None => Ok((current_uid, current_gid)),
        Some((u, None)) => Ok((uid_from_str(u)?, current_gid)),
        Some((u, Some(g))) => Ok((uid_from_str(u)?, gid_from_str(g)?)),
    }
}

impl TryFrom<&RuntimeConfig> for ExecConfig {
    type Error = Error;

    fn try_from(value: &RuntimeConfig) -> std::result::Result<Self, Self::Error> {
        let argv = parse_argv(value)?;
        let env = parse_env(value);
        let (uid, gid) = parse_uid_gid(value)?;

        let arg0 = PathBuf::from_str(&argv[0])
            .map_err(|e| OciLauncherError::PathConversionError(e, argv[0].clone()))?;

        let cwd = match value.working_dir().as_ref() {
            Some(path) => Some(
                PathBuf::from_str(path)
                    .map_err(|e| OciLauncherError::PathConversionError(e, path.clone()))?,
            ),
            None => None,
        };

        Ok(ExecConfig {
            exec: arg0,
            argv,
            envp: env,
            uid,
            gid,
            chroot: None,
            chdir: cwd,
        })
    }
}

fn hash_config(config: &[u8]) -> Vec<u8> {
    let mut hash = Sha384::new();
    hash.update(config);
    hash.finalize().to_vec()
}

enum Action {
    Install(ImageInfo, Vec<Vec<u8>>, Vec<u8>),
    LaunchInstalled(Box<Metadata>)
}

#[async_trait]
impl Launcher for OciLauncher {
    async fn install(
        &mut self,
        path: &Path,
        im_url: &str,
        name: &str,
        version: &str,
    ) -> Result<ApplicationMetadata> {
        debug!("Installing {}:{} from {}", name, version, im_url);
        let oci_client = self.create_oci_client(im_url)?;

        let reference = Reference::try_from(version)
            .map_err(|e| OciLauncherError::InvalidVersion(e, version.to_owned()))?;

        info!("Fetching image info for {}:{}", name, version);
        let try_image_info = oci_client
            .get_image_info(name, reference)
            .await;

        let try_current_metadata = self.try_read_metadata(path).await;

        let action = match (try_image_info, try_current_metadata) {
            (Ok(image_info), try_metadata) => {
                let annotations = RequiredAnnotations::try_from(
                    image_info
                        .annotations()
                        .ok_or(OciLauncherError::AppImageLacksAnnotations())?
                )?;
                let new_vendor_cert = vec![annotations.vendor_pubkey.clone(), annotations.vendor_pubkey_signature.clone()];
                let new_config_hash = hash_config(image_info.config_bytes());

                let is_installed_newest = try_metadata.filter(|metadata| metadata.config_hash == new_config_hash && metadata.vendor_cert == new_vendor_cert);

                match is_installed_newest {
                    None => {
                        info!("Verifying application {}:{} signature", name, version);
                        self.verify_signature(&annotations, image_info.config_bytes())?;
                        info!("Signature validated successfully");

                        Action::Install(image_info, new_vendor_cert, new_config_hash)
                    },
                    Some(metadata) => {
                        Action::LaunchInstalled(Box::new(metadata))
                    }
                }

            }

            (Err(e), Some(metadata)) => {
                warn!("Failed to reach image registry server: {:?}", e);

                Action::LaunchInstalled(Box::new(metadata))
            }

            (Err(e), None) => {
                error!("Failed to reach image registry server: {:?}", e);
                return Err(OciLauncherError::ImageInfoFetching(e).into());
            }
        };

        match action {
            Action::Install(image_info, new_vendor_cert, new_config_hash) => {
                let img_dir = path.join("img");
                let temp_dir = path.join("temp");
                let unpack_dir = path.join("unpack");

                mkdirp(&temp_dir).await?;
                mkdirp(&unpack_dir).await?;

                info!(
                    "Unpacking application to {:?} using {:?} as temp dir",
                    unpack_dir, temp_dir
                );
                let result = oci_client
                    .unpack_image(&image_info, &unpack_dir, &temp_dir)
                    .await;

                let result = match result {
                    Err(e) => Err(OciLauncherError::UnpackError(e).into()),
                    Ok(()) => {
                        info!(
                            "Installation finished, moving {:?} to {:?}",
                            unpack_dir, img_dir
                        );
                        self.move_unpack_to_img(&unpack_dir, &img_dir).await
                    }
                };

                info!("Cleaning up, removing {:?}", temp_dir);
                let _ = rmrf(&temp_dir)
                    .await
                    .map_err(|e| error!("Failed to cleanup {:?}, error {:?}", temp_dir, e));
                info!("Cleaning up, removing {:?}", unpack_dir);
                let _ = rmrf(&unpack_dir)
                    .await
                    .map_err(|e| error!("Failed to cleanup {:?}, error {:?}", unpack_dir, e));

                result.map_err(|e| {
                    error!(
                        "Failed to install applcation {}:{}, reason: {:?}",
                        name, version, e
                    );
                    e
                })?;

                let image_config = json_load_bytes(image_info.config_bytes())?;

                self.save_metadata(
                    path,
                    Metadata {
                        vendor_cert: new_vendor_cert.clone(),
                        config_hash: new_config_hash.clone(),
                        image_config
                    },
                )
                .await?;

                Ok(ApplicationMetadata {
                    vendor_data: new_vendor_cert,
                    image_hash: new_config_hash
                })

            },

            Action::LaunchInstalled(metadata) => {
                info!("Launching already installed application from disk");

                Ok(ApplicationMetadata {
                    vendor_data: metadata.vendor_cert.clone(),
                    image_hash: metadata.config_hash.clone()
                })
            }
        }
    }

    async fn prepare(&mut self, path: &Path) -> Result<Box<dyn ApplicationHandler + Send + Sync>> {
        let metadata = self
            .try_read_metadata(path)
            .await
            .ok_or(OciLauncherError::MetadataNotFound())?;

        let runtime_config = metadata
            .image_config
            .config()
            .as_ref()
            .ok_or(OciLauncherError::RuntimeConfigMissing())?;

        let mut exec_config = ExecConfig::try_from(runtime_config)?;
        let img_dir = path.join("img");
        exec_config.chroot = Some(img_dir);

        debug!("Launching from config: {:?}", exec_config);

        Ok(Box::new(SimpleApplicationHandler::new(exec_config)))
    }
}
