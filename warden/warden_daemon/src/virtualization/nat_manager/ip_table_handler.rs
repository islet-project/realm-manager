use thiserror::Error;

#[derive(Error, Debug)]
pub enum IpTableHandlerError {
    #[error("Failed to create handler: {0}")]
    HandlerError(String),
    #[error(
        "Failed to insert rule to chain: {chain_name} in table: {table_name} message: {message}"
    )]
    RuleAdd {
        chain_name: String,
        table_name: String,
        message: String,
    },
    #[error("Failed to insert chain: {chain_name} to table: {table_name} message: {message}")]
    ChainAdd {
        chain_name: String,
        table_name: String,
        message: String,
    },
    #[error("Failed to remove chain: {chain_name} from table: {table_name} message: {message}")]
    ChainRemove {
        chain_name: String,
        table_name: String,
        message: String,
    },
}

pub trait IpTableHandler {
    fn insert_ip_table_rules(&self) -> Result<(), IpTableHandlerError>;
    fn remove_ip_table_rules(&self) -> Result<(), IpTableHandlerError>;
}
