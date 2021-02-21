use crate::gc::heap::Marking;

pub trait MarkingConstraint {
    fn name(&self) -> &str {
        "<anonymous name>"
    }
    fn execute(&mut self, marking: &mut Marking);
}

pub struct SimpleMarkingConstraint {
    name: String,
    exec: Box<dyn FnMut(&mut Marking)>,
}
impl SimpleMarkingConstraint {
    pub fn new(name: &str, exec: impl FnMut(&mut Marking) + 'static) -> Self {
        Self {
            name: name.to_owned(),
            exec: Box::new(exec),
        }
    }
}
impl MarkingConstraint for SimpleMarkingConstraint {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, marking: &mut Marking) {
        (self.exec)(marking);
    }
}
