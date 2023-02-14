use crate::errors::GenericError;

pub trait Command {
    fn execute(&self) -> Result<(), GenericError>;
}

