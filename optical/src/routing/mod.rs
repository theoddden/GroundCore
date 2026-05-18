// Optical-aware routing
//
// Routing for optical links is fundamentally different from RF routing. Optical
// links are binary (acquired or not), have limited field-of-regard, and require
// coordinated acquisition timing.

pub mod handoff;
pub mod path_finder;
pub mod topology;

pub use handoff::{HandoffStrategy, HandoffTrigger};
pub use path_finder::{OpticalRoute, PathFinder, RouteConstraints};
pub use topology::{OpticalNode, OpticalTopology, TopologyEdge};
