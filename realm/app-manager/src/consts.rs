use hex_literal::hex;

pub const APPLICATION_SLK_SALT: [u8; 32] =
    hex!("8a3bda84cdbc4b862eddd8327a08e27eb6c8f0209676ff90d886862369bf874c");

pub const RSI_KO: &str = "/lib/modules/rsi.ko";

pub const ANNOTATION_SIGNATURE: &str = "com.samsung.islet.image.signature";
pub const ANNOTATION_VENDORPUB: &str = "com.samsung.islet.image.vendorpub";
pub const ANNOTATION_VENDORPUB_SIGNATURE: &str = "com.samsung.islet.image.vendorpub.signature";
