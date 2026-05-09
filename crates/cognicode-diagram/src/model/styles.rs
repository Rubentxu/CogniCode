//! C4 Styles — element styling

use serde::{Deserialize, Serialize};

/// Shape of an element in the diagram
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Box,
    RoundedBox,
    Circle,
    Cylinder,
    Person,
    Folder,
    Hexagon,
}

/// Style for a C4 element type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementStyle {
    pub tag: String,
    pub shape: Shape,
    pub background: String,
    pub color: String,
    pub border: String,
    pub font_size: u32,
}

/// Theme for the entire diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub element_styles: Vec<ElementStyle>,
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            name: "default-dark".into(),
            element_styles: vec![
                ElementStyle {
                    tag: "Software System".into(),
                    shape: Shape::RoundedBox,
                    background: "#1168bd".into(),
                    color: "#ffffff".into(),
                    border: "#1168bd".into(),
                    font_size: 14,
                },
                ElementStyle {
                    tag: "Container".into(),
                    shape: Shape::RoundedBox,
                    background: "#438dd5".into(),
                    color: "#ffffff".into(),
                    border: "#438dd5".into(),
                    font_size: 14,
                },
                ElementStyle {
                    tag: "Component".into(),
                    shape: Shape::RoundedBox,
                    background: "#85bbf0".into(),
                    color: "#000000".into(),
                    border: "#85bbf0".into(),
                    font_size: 14,
                },
                ElementStyle {
                    tag: "DataStore".into(),
                    shape: Shape::Cylinder,
                    background: "#438dd5".into(),
                    color: "#ffffff".into(),
                    border: "#438dd5".into(),
                    font_size: 14,
                },
                ElementStyle {
                    tag: "Person".into(),
                    shape: Shape::Person,
                    background: "#08427b".into(),
                    color: "#ffffff".into(),
                    border: "#08427b".into(),
                    font_size: 14,
                },
            ],
        }
    }
}
