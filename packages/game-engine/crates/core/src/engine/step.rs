//! Rule execution. `advance` runs atomic rule steps within a budget; each
//! step works on a staged copy, lets the suspended frame select the routine
//! that advances it, and commits only if the step and its publication succeed.
mod advance;
mod dispatch;
mod resolve;
