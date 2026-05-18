// Optical-aware routing
//
// Routing for optical links is fundamentally different from RF routing. Optical
// links are binary (acquired or not), have limited field-of-regard, and require
// coordinated acquisition timing.

pub mod topology;
pub mod path_finder;
pub mod handoff;

pub use topology::{OpticalTopology, TopologyEdge, OpticalNode};
pub use path_finder::{PathFinder, OpticalRoute, RouteConstraints};
pub use handoff::{HandoffStrategy, HandoffTrigger};
