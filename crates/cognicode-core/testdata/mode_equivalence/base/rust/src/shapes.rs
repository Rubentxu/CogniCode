pub struct Circle {
    pub radius: f64,
}

pub enum Kind {
    Round,
    Square,
}

pub trait Area {
    fn area(&self) -> f64;
}

impl Area for Circle {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

pub fn unit_circle() -> Circle {
    Circle { radius: 1.0 }
}
