use crate::traits::PoolTransaction;
use std::{cmp::Ordering, fmt::Debug, marker::PhantomData};

/// Priority of the transaction that can be missing.
///
/// Transactions with missing priorities are ranked lower.
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Priority<T: Ord + Clone> {
    /// The value of the priority of the transaction.
    Value(T),
    /// Missing priority due to ordering internals.
    None,
}

impl<T: Ord + Clone> From<Option<T>> for Priority<T> {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::None, Priority::Value)
    }
}

impl<T: Ord + Clone> PartialOrd for Priority<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Clone> Ord for Priority<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Value(a), Self::Value(b)) => a.cmp(b),
            // Note: None should be smaller than Value.
            (Self::Value(_), Self::None) => Ordering::Greater,
            (Self::None, Self::Value(_)) => Ordering::Less,
            (Self::None, Self::None) => Ordering::Equal,
        }
    }
}

/// Transaction ordering trait to determine the order of transactions.
///
/// Decides how transactions should be ordered within the pool, depending on a `Priority` value.
///
/// The returned priority must reflect [total order](https://en.wikipedia.org/wiki/Total_order).
pub trait TransactionOrdering: Debug + Send + Sync + 'static {
    /// Priority of a transaction.
    ///
    /// Higher is better.
    type PriorityValue: Ord + Clone + Default + Debug + Send + Sync;

    /// The transaction type to determine the priority of.
    type Transaction: PoolTransaction;

    /// Returns the priority score for the given transaction.
    fn priority(
        &self,
        transaction: &Self::Transaction,
        base_fee: u64,
    ) -> Priority<Self::PriorityValue>;
}

/// Default ordering for the pool.
///
/// The transactions are ordered by their coinbase tip.
/// The higher the coinbase tip is, the higher the priority of the transaction.
#[derive(Debug)]
#[non_exhaustive]
pub struct CoinbaseTipOrdering<T>(PhantomData<T>);

impl<T> TransactionOrdering for CoinbaseTipOrdering<T>
where
    T: PoolTransaction + 'static,
{
    type PriorityValue = u128;
    type Transaction = T;

    /// Source: <https://github.com/ethereum/go-ethereum/blob/7f756dc1185d7f1eeeacb1d12341606b7135f9ea/core/txpool/legacypool/list.go#L469-L482>.
    ///
    /// NOTE: The implementation is incomplete for missing base fee.
    fn priority(
        &self,
        transaction: &Self::Transaction,
        base_fee: u64,
    ) -> Priority<Self::PriorityValue> {
        transaction.effective_tip_per_gas(base_fee).into()
    }
}

impl<T> Default for CoinbaseTipOrdering<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> Clone for CoinbaseTipOrdering<T> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// Insertion order based ordering for the pool.
///
/// This ordering returns a constant priority value for all transactions,
/// which means transactions will be ordered solely by their `submission_id`
/// (the order in which they were submitted to the pool).
///
/// When two transactions have the same priority, the pool falls back to
/// ordering by submission_id, where older transactions (lower submission_id)
/// have higher priority. Since all transactions get the same priority with
/// this ordering, they will always be processed in insertion order.
#[derive(Debug)]
#[non_exhaustive]
pub struct InsertionOrdering<T>(PhantomData<T>);

impl<T> TransactionOrdering for InsertionOrdering<T>
where
    T: PoolTransaction + 'static,
{
    type PriorityValue = u128;
    type Transaction = T;

    /// Returns a constant priority of 0 for all transactions.
    ///
    /// This ensures all transactions have equal priority, causing the pool
    /// to fall back to ordering by submission_id (insertion order).
    fn priority(
        &self,
        _transaction: &Self::Transaction,
        _base_fee: u64,
    ) -> Priority<Self::PriorityValue> {
        Priority::Value(0)
    }
}

impl<T> Default for InsertionOrdering<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> Clone for InsertionOrdering<T> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// A configurable transaction ordering that can switch between
/// coinbase tip ordering and insertion ordering.
///
/// This enum allows the pool to use different ordering strategies
/// while maintaining a consistent type signature.
#[derive(Debug, Clone)]
pub enum TxPoolOrdering<T> {
    /// Orders transactions by their coinbase tip (default behavior).
    CoinbaseTip(CoinbaseTipOrdering<T>),
    /// Orders transactions by their insertion order.
    Insertion(InsertionOrdering<T>),
}

impl<T: PoolTransaction + 'static> TransactionOrdering for TxPoolOrdering<T> {
    type PriorityValue = u128;
    type Transaction = T;

    fn priority(
        &self,
        transaction: &Self::Transaction,
        base_fee: u64,
    ) -> Priority<Self::PriorityValue> {
        match self {
            Self::CoinbaseTip(ordering) => ordering.priority(transaction, base_fee),
            Self::Insertion(ordering) => ordering.priority(transaction, base_fee),
        }
    }
}

impl<T> Default for TxPoolOrdering<T> {
    fn default() -> Self {
        Self::CoinbaseTip(CoinbaseTipOrdering::default())
    }
}

impl<T> TxPoolOrdering<T> {
    /// Creates a new ordering based on whether insertion order should be preserved.
    pub fn new(preserve_insertion_order: bool) -> Self {
        if preserve_insertion_order {
            Self::Insertion(InsertionOrdering::default())
        } else {
            Self::CoinbaseTip(CoinbaseTipOrdering::default())
        }
    }

    /// Creates a coinbase tip ordering (default).
    pub fn coinbase_tip() -> Self {
        Self::CoinbaseTip(CoinbaseTipOrdering::default())
    }

    /// Creates an insertion ordering.
    pub fn insertion() -> Self {
        Self::Insertion(InsertionOrdering::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        let p1 = Priority::Value(3);
        let p2 = Priority::Value(1);
        let p3 = Priority::None;

        assert!(p1 > p2); // 3 > 1
        assert!(p1 > p3); // Value(3) > None
        assert!(p2 > p3); // Value(1) > None
        assert_eq!(p3, Priority::None);
    }
}
