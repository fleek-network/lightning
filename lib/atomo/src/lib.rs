pub mod context;
pub mod db;
pub mod gc_list;
pub mod mt;
pub mod once_ptr;
pub mod serder;
pub mod shared;
pub mod snapshot;

pub use context::Context;
pub use db::{Atomo, QueryHalf, UpdateHalf};
pub use serder::SerdeBackend;
pub use shared::Shared;

pub type DefaultSerdeBackend = serder::BincodeSerde;

#[cfg(test)]
mod tests {
    use crate::Atomo;

    #[test]
    fn x() {
        let (mut u, q) = Atomo::<String, u128>::new().split();

        u.run(|c| {
            c.insert("Alice".into(), 0);
        });

        let q_tmp = q.clone();
        let t0 = std::thread::spawn(move || {
            let q = q_tmp;
            for _ in 0..11 {
                q.run(|c| {
                    let s = c.get(&"Alice".into()).unwrap();
                    println!("[t0-S] Balance= {s}");
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let e = c.get(&"Alice".into()).unwrap();
                    println!("[t0-E] Balance= {e}");
                    assert_eq!(s, e);

                    q.run(|c| {
                        let n = c.get(&"Alice".into()).unwrap();
                        println!("[t0-A] Balance= {n}");
                    })
                });
            }
        });

        let q_tmp = q.clone();
        let t1 = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let q = q_tmp;
            for _ in 0..11 {
                q.run(|c| {
                    let s = c.get(&"Alice".into()).unwrap();
                    println!("[t1-S] Balance= {s}");
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let e = c.get(&"Alice".into()).unwrap();
                    println!("[t1-E] Balance= {e}");
                    assert_eq!(s, e);

                    q.run(|c| {
                        let n = c.get(&"Alice".into()).unwrap();
                        println!("[t1-A] Balance= {n}");
                        // this assertion can fail on systems with only a few physical CPUs.
                        // The reason being that in case there is for example only one physical
                        // CPU, each function inside a thread is executed kind of in-order.
                        // So this implies that other update calls might not have been executed
                        // just yet, so the case of `n = e` can happen in those situations.
                        // assert!(n > e);
                    })
                });
            }
        });

        for i in 1..100 {
            std::thread::sleep(std::time::Duration::from_millis(20));

            u.run(|c| {
                c.insert("Alice".into(), i);
            });

            q.run(|c| {
                let b = c.get(&"Alice".into()).unwrap();
                println!("[u] Balance= {b}");
            })
        }

        t0.join().unwrap();
        t1.join().unwrap();
    }
}
