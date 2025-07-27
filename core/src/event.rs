/// Basic UI events used for widgets
#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    PointerDown { x: i32, y: i32 },
    PointerUp { x: i32, y: i32 },
    PointerMove { x: i32, y: i32 },
}
