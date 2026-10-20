//! Physical diagrams: the infrastructure. Networks divided into zones, the users, servers,
//! databases, appliances, and storage inside them, and the traffic between them with its
//! protocol and port. See `docs/diagrams/physical.md`.

pub mod model;
pub mod rules;
pub mod scene;
#[cfg(feature = "ui")]
pub mod ui;

pub use model::{Physical, PhysicalArrow, PhysicalColors, PhysicalKind, PhysicalProps};

/// The editor's instructions, shown above the canvas.
pub const HELP: &str = "Drag an element onto the grid, or select it to add it. Move a placed element by dragging it, or focus it and use the arrow keys (hold Shift for bigger steps). Press Enter to edit its properties and Delete to remove it. Select more than one element by holding Shift, Ctrl, or Cmd as you click each, or by focusing each and pressing Shift+Space, to move them together by dragging or with the arrow keys, or to remove them together with Delete. Zones fill the network they're in: a zone dropped onto a network divides it under its other zones, resizing a network resizes its zones with it, and a new network starts with the same zones as the first. A zone stays in its network: drag it up or down, or use the up and down arrow keys, to swap it with the zone above or below. Removing a network's last zone removes the network too. Users, servers, databases, load balancers, gateways, and storage go inside zones, and stay within the room a zone has, so it keeps its size: with the arrow keys, one at a zone's edge moves across it into the next. Firewalls stand on their own, so one can sit across the edge between two zones or two networks, and doesn't move with them. Resize a network or zone with the handles on its top-left and bottom-right corners, by dragging or with the arrow keys. To draw an arrow between users, servers, databases, firewalls, load balancers, gateways, and storage, drag from the round handle on the selected element, or use Add arrow in its properties. Every arrow shows its protocol and port on the middle of its line: select the arrow to change them. In a server's properties, choose whether it's physical, a virtual machine, or a container, and how many instances of it run, shown beside it as ×3 when more than one. In a database's properties, choose which database it is: its abbreviation shows on its figure, and new arrows to it start with TCP on its port, and how many nodes it has, shown the same way. Scroll the canvas by dragging an empty part of it, or focus it and use the arrow keys. Zoom with the Zoom slider in its bottom-right corner, or with Cmd or Ctrl and the scroll wheel.";
