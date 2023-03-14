/// Gives additional information about the negative outcome of a batch
/// cell decision.
///
/// Since batch queries can be made for batch sizes bigger than the
/// rate limiter parameter could accomodate, there are now two
/// possible negative outcomes:
///
///   * `BatchNonConforming` - the query is valid but the Decider can
///     not accomodate them.
///
///   * `InsufficientCapacity` - the query was invalid as the rate
///     limite parameters can never accomodate the number of cells
///     queried for.
#[derive(Debug, PartialEq)]
pub enum NegativeMultiDecision<E> {
    /// A batch of cells (the first argument) is non-conforming and
    /// can not be let through at this time. The second argument gives
    /// information about when that batch of cells might be let
    /// through again (not accounting for thundering herds and other,
    /// simultaneous decisions).
    BatchNonConforming(u32, E),

    /// The number of cells tested (the first argument) is larger than
    /// the bucket's capacity, which means the decision can never have
    /// a conforming result. The argument gives the maximum number of
    /// cells that could ever have a conforming result.
    InsufficientCapacity(u32),
}
