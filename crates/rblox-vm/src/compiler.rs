use crate::chunk::{Chunk, OP_CONSTANT, OP_RETURN, Value};

pub fn compile(source: &str) -> Result<Chunk, String> {
    let number = source.parse::<f64>().map_err(|_| {
        format!(
            "Compile error: only number literals are supported for now (got '{}').",
            source
        )
    })?;

    let mut chunk = Chunk::new();
    let index = chunk.add_constant(Value::Number(number));
    chunk.write(OP_CONSTANT, 1);
    chunk.write(index, 1);
    chunk.write(OP_RETURN, 1);
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_accepts_number_literal() {
        let chunk = compile("3.14").expect("number literal should compile");
        assert_eq!(chunk.code.len(), 3);
    }

    #[test]
    fn compile_rejects_non_number() {
        let result = compile("1 + 2");
        assert!(result.is_err());
    }
}
