use crate::Result;
use serde_json::Value;
use std::collections::HashMap;

pub type RawArgs = HashMap<String, Value>;

pub trait FromArgs: Sized {
    fn from_args(args: &RawArgs) -> Result<Self>;
}

impl FromArgs for () {
    fn from_args(_args: &RawArgs) -> Result<Self> {
        Ok(())
    }
}

impl FromArgs for RawArgs {
    fn from_args(args: &RawArgs) -> Result<Self> {
        Ok(args.clone())
    }
}
