//! TODO(qti3e): Design the new SDK and service loader interface.
//!
//! Since the previous SDK interface was implemented with the assumption that node
//! operators are required to 'opt-in' to the new services, and therefore trusting
//! the service creator, it's designed is now rendered useless.
//!
//! The new SDK requires another way for services to operate on a node without having
//! a way to act maliciously, and any commitment a service needs to make involving a
//! signature from the node must be first validated by the core.
//!
//! It is within the design requirements of the new SDK and service loader to not allow
//! malicious service creators to abuse their degree of freedom in order to cause an
//! honest node to get slashed.
//!
//! Another important factor to notice is that now the primary way for a service to
//! communicate to the node is through the IPC using message passing, this requires
//! us to provide efficient APIs for message passing as well as direct write pipelines
//! similar to Linux's 'splice' functionality (when used as a transfer mechanism.)
//!
//! Also we should keep in mind the requirement from certain service that rely on
//! quorum based slash, the support of these kind of slash is tricky and requires
//! careful consideration before starting any implementation, I personally still
//! have to figure out the implications of having (or abandoning) such functionality.
//!
//! Group compute is also another important notion when it comes to many uses cases
//! of services, these involve services that require interaction between a group of
//! nodes in order to fulfill a certain task. This should be easily do-able with the
//! provided methods.
//!
//! Another important requirement (this one is a given) is the service's ability to
//! the block store as well as storage on the node. A service should have easy access
//! to blockstore and requesting data from the node as well as having access to storage
//! provided by a node. The raw storage on a single node *MUST* be assumed faulty and
//! not to be trusted by service creators and if a service requires to store something
//! for persistence purposes there may be some valid path forward:
//!
//! 1. Building a fault-tolerant storage on Fleek Network utilizing many nodes.
//! 2. Integration with FIL/Arwave or other potential storage protocols.
//! 3. Use of one pinning service that is built on Fleek Network as a building-block
//!   by other services.
//!
//! Requirements:
//!  - Arbitrary 'pure-function' commitments.
//!  - Efficient message passing to core.
//!  - Direct resource pipelines.
//!  - Secure service specific quorum slashing.
//!  - Node clustering and group compute helpers.
//!  - 'Fleek Network File System' read access. (aka blockstore)
//!  - Disk space for persistence.
