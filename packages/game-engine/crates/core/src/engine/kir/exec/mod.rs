//! One executor module per kernel verb the interpreter performs directly.
//! Verbs that keep an established executor (targeted removal, damage,
//! tokens, player actions) are dispatched by `execute_kernel` to it.
mod attach;
mod combat;
mod continuous;
mod draw;
mod grant_keywords;
mod library;
mod life;
mod set_move;
mod tap;
