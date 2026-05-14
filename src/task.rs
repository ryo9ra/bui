// Worker-thread plumbing for async git operations.
//
// v0.1 calls Repo methods synchronously from the main thread (see
// `App::initial_refresh`). The worker abstraction is filled in when fetch /
// pull / push land in v0.2; the Event::TaskResult variant and the trait
// shapes are already in place so the rest of the app does not need to
// change.
