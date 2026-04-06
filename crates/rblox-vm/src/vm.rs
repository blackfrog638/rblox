use crate::chunk::{Chunk, OP_CONSTANT, OP_RETURN, Value};

pub struct VM {
    chunk: Chunk,
    ip: usize,
}

impl VM {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
            ip: 0,
        }
    }

    pub fn interpret(&mut self, chunk: Chunk) -> Result<(), String> {
        self.chunk = chunk;
        self.ip = 0;

        self.run()
    }

    pub fn run(&mut self) -> Result<(), String> {
        loop {
            let instruction = self.read_byte()?;
            match instruction {
                OP_RETURN => {
                    return Ok(());
                }
                OP_CONSTANT => {
                    let constant = self.read_constant()?;
                    println!("{}", constant);
                }
                _ => {
                    return Err(format!(
                        "Unknown opcode {} at offset {}",
                        instruction,
                        self.ip.saturating_sub(1)
                    ));
                }
            }
        }
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        let byte =
            self.chunk.code.get(self.ip).copied().ok_or_else(|| {
                format!("Instruction pointer out of bounds at offset {}", self.ip)
            })?;
        self.ip += 1;
        Ok(byte)
    }

    fn read_constant(&mut self) -> Result<Value, String> {
        let constant_index = self.read_byte()?;
        self.chunk
            .constants
            .get(constant_index as usize)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Invalid constant index {} at offset {}",
                    constant_index,
                    self.ip.saturating_sub(1)
                )
            })
    }
}
