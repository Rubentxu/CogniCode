pub struct Disc {
    pub radius: f64,
}

pub enum Form {
    Round,
    Square,
}

pub trait Measure {
    fn area(&self) -> f64;
}

impl Measure for Disc {
    fn area(&self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

pub fn unit_disc() -> Disc {
    Disc { radius: 1.0 }
}
