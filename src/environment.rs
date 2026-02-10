use crate::token::Literal;
pub struct Environment {
    values: std::collections::HashMap<String, Literal>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            values: std::collections::HashMap::new(),
        }
    }

    pub fn define(&mut self, name: String, value: Literal) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<&Literal> {
        self.values.get(name)
    }
}
