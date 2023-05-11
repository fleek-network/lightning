use std::ops::Deref;

pub trait TableRef<K, V> {
    fn set(&self, key: K, value: V);

    fn get(&self, key: &K) -> Option<V>;
}

pub trait Value<T>: Deref<Target = T> {}
impl<T> Value<T> for &T {}

pub trait ApplicationState {
    type AccountInfoRef: TableRef<PublicKey, AccountInfo>;
    type BalanceInfoRef: TableRef<PublicKey, u128>;

    fn account_info_ref(&self) -> Self::AccountInfoRef;
    fn balance_info_ref(&self) -> Self::BalanceInfoRef;
}

#[derive(Hash, Eq, PartialEq)]
pub struct PublicKey(u8);

#[derive(Clone)]
pub struct AccountInfo;

pub mod hashmap_backend {
    use super::*;
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::sync::Arc;

    use super::ApplicationState;

    #[derive(Default)]
    pub struct HashMapBackend {
        balances: HashMapTable<PublicKey, u128>,
    }

    pub struct HashMapTable<K: Send, V: Send>(Arc<std::sync::Mutex<HashMap<K, V>>>);

    impl<K: Send, V: Send> Default for HashMapTable<K, V> {
        fn default() -> Self {
            Self(Arc::new(std::sync::Mutex::new(HashMap::new())))
        }
    }

    impl ApplicationState for HashMapBackend {
        type BalanceInfoRef = HashMapTable<PublicKey, u128>;
        type AccountInfoRef = HashMapTable<PublicKey, AccountInfo>;

        fn account_info_ref(&self) -> Self::AccountInfoRef {
            todo!()
        }

        fn balance_info_ref(&self) -> Self::BalanceInfoRef {
            HashMapTable(self.balances.0.clone())
        }
    }

    impl<K: Send, V: Send> TableRef<K, V> for HashMapTable<K, V>
    where
        K: Eq + Hash,
        V: Clone,
    {
        fn set(&self, key: K, value: V) {
            let mut lock = self.0.lock().unwrap();
            lock.insert(key, value);
        }

        fn get(&self, key: &K) -> Option<V> {
            let lock = self.0.lock().unwrap();
            lock.get(key).cloned()
        }
    }
}

pub mod implemntation {
    use super::*;

    pub struct ApplicationLogic<T: ApplicationState>(pub T);

    impl<T: ApplicationState> ApplicationLogic<T> {
        pub fn mint(&self, user_id: PublicKey, amount: u128) {
            let balances_ref = self.0.balance_info_ref();

            let current_balance = balances_ref.get(&user_id).unwrap_or(0);
            let new_balance = current_balance + amount;

            balances_ref.set(user_id, new_balance);
        }

        pub fn get_balance(&self, user_id: PublicKey) -> u128 {
            let balances_ref = self.0.balance_info_ref();
            balances_ref.get(&user_id).unwrap_or(0)
        }
    }
}

#[test]
fn playground() {
    let app = implemntation::ApplicationLogic(hashmap_backend::HashMapBackend::default());

    app.mint(PublicKey(12), 100);
    dbg!(app.get_balance(PublicKey(12)));
}
