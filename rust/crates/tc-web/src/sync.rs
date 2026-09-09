//! Getting the queue to the server, and resolving what happened while it waited.
//!
//! The interesting case is not "send the edits" - it is "somebody else sent theirs first".
//! Because the queue holds intentions rather than finished tours, that case has an answer
//! better than an apology: take their tour, replay ours on top, and try again. Both sets of
//! edits survive, and nobody is told to redo their work.
//!
//! Only a genuine collision - two people editing the same expense - can still lose
//! something, and it loses it the way any last-writer-wins system does. The alternative is
//! asking a person on a train to merge a JSON document, which is not an alternative.

use crate::api::{self, SaveError};
use crate::queue::{self, Operation};
use tc_core::Tour;

/// How the last attempt to reach the server went.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Everything is on the server.
    Synced,
    /// Nothing to send, and nothing was tried.
    Idle,
    /// This many edits are still waiting, because there is no network.
    Waiting(usize),
    /// Something went wrong that waiting will not fix.
    Failed(String),
}

/// The number of times a save may be beaten by somebody else before giving up.
///
/// Each round takes their tour and replays ours onto it, so a round only repeats if another
/// save landed in the meantime. Three is generous; the shape of the loop matters more than
/// the number, since without a limit two busy clients could keep each other going forever.
const MAX_ROUNDS: usize = 3;

/// Sends whatever is queued for this tour, and answers with the tour to build the screen
/// from.
///
/// What comes back is the **base** - the server's copy, or the last one this device saw -
/// with nothing from the queue applied. Applying the queue is the caller's job and happens
/// in exactly one place ([`queue::with_pending`]); doing it here as well is how an edit
/// once appeared twice on screen.
pub async fn push(tour_id: &str) -> (Option<Tour>, Status) {
    let ops = queue::pending(tour_id);

    // Nothing of ours to send: just take what the server has.
    if ops.is_empty() {
        return match api::tour(tour_id).await {
            Ok(t) => {
                queue::cache(&t);
                (Some(t), Status::Idle)
            }
            Err(e) if api::looks_offline(&e) => (queue::cached(tour_id), Status::Waiting(0)),
            Err(e) => (queue::cached(tour_id), Status::Failed(e)),
        };
    }

    for _ in 0..MAX_ROUNDS {
        // Always start from the freshest server copy: that is what makes a conflict
        // recoverable instead of fatal.
        let server = match api::tour(tour_id).await {
            Ok(t) => t,
            Err(e) if api::looks_offline(&e) => {
                return (queue::cached(tour_id), Status::Waiting(ops.len()));
            }
            Err(e) => return (queue::cached(tour_id), Status::Failed(e)),
        };

        let next = replay(&ops, &server);

        match api::save_tour(&next).await {
            Ok(()) => {
                queue::set_pending(tour_id, &[]);
                // Read back rather than assume: the server assigns the new state id, and
                // the next save has to carry it.
                return match api::tour(tour_id).await {
                    Ok(t) => {
                        queue::cache(&t);
                        (Some(t), Status::Synced)
                    }
                    Err(_) => (Some(next), Status::Synced),
                };
            }
            // Somebody was quicker. Their tour is now the base; go round again.
            Err(SaveError::Conflict) => continue,
            // The read worked and the write did not, so the freshest base is the one just
            // read - not the older copy in the cache.
            Err(SaveError::Offline(_)) => {
                queue::cache(&server);
                return (Some(server), Status::Waiting(ops.len()));
            }
            Err(SaveError::Other(e)) => return (Some(server), Status::Failed(e)),
        }
    }

    (
        queue::cached(tour_id),
        Status::Failed(format!(
            "Could not save: the tour kept changing underneath ({MAX_ROUNDS} tries)."
        )),
    )
}

/// The tour with every queued operation carried out, in the order they were made.
fn replay(ops: &[Operation], base: &Tour) -> Tour {
    ops.iter().fold(base.clone(), |acc, op| op.apply(&acc))
}

/// Records an edit and tries to send it.
///
/// The edit is written down first and sent second, which is the whole difference between an
/// app that works on a train and one that does not.
pub async fn record(tour_id: &str, op: Operation) -> (Option<Tour>, Status) {
    queue::push(tour_id, op);
    push(tour_id).await
}
