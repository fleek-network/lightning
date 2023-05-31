// use std::{
//     hint::black_box,
//     sync::atomic::{AtomicBool, AtomicUsize},
//     time::Duration,
// };
//
// use atomo::{Atomo, Context, SerdeBackend};
// use rand::Rng;
//
// type UserId = u8;
//
// fn mint<S: SerdeBackend>(ctx: &mut Context<UserId, u128, S>, user: UserId, amount: u128) -> u128
// {     let balance = ctx.get(&user).map(|s| *s).unwrap_or(0);
//     ctx.insert(user, balance + amount);
//     balance + amount
// }
//
// fn balance<S: SerdeBackend>(ctx: &mut Context<UserId, u128, S>, user: UserId) -> u128 {
//     ctx.get(&user).map(|s| *s).unwrap_or(0)
// }
//
// fn main() {
//     let (mut u, q) = Atomo::<UserId, u128>::new().split();
//
//     let num_query_threads: usize = get_arg("-t").unwrap_or(4);
//     let num_query_per_thread: usize = get_arg("-q").unwrap_or(1_000_000);
//     let num_mint_per_update: usize = get_arg("-u").unwrap_or(16);
//
//     println!("To configure use `-t [num threads] -q [num query per thread] -u [num tx per
// update]");     println!("STARTING BENCHMARK");
//     println!("NUM THREADS      = {}", num_query_threads);
//     println!("NUM QUERY/THREAD = {}", num_query_per_thread);
//     println!("NUM TX/UPDATE = {}", num_mint_per_update);
//     println!("\n");
//
//     static PENDING: AtomicUsize = AtomicUsize::new(0);
//     static START: AtomicBool = AtomicBool::new(false);
//
//     PENDING.store(num_query_threads, std::sync::atomic::Ordering::Relaxed);
//
//     let mut handles = Vec::<std::thread::JoinHandle<Duration>>::new();
//
//     for _ in 0..num_query_threads {
//         let q = q.clone();
//         let handle = std::thread::spawn(move || {
//             while !START.load(std::sync::atomic::Ordering::Relaxed) {
//                 std::hint::spin_loop()
//             }
//
//             let mut rng: u32 = rand::thread_rng().gen();
//
//             let now = std::time::Instant::now();
//
//             for _ in 0..num_query_per_thread {
//                 rng = rng.wrapping_mul(48271) % 0x7fffffff;
//                 q.run(|ctx| black_box(balance(ctx, rng as u8)));
//             }
//
//             let duration = now.elapsed();
//             PENDING.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
//             duration
//         });
//
//         handles.push(handle);
//     }
//
//     let mut rng: u32 = rand::thread_rng().gen();
//
//     // signal every threads that's waiting to start.
//     START.store(true, std::sync::atomic::Ordering::Relaxed);
//
//     let now = std::time::Instant::now();
//     let updates = if num_query_threads == 0 {
//         for _ in 0..num_query_per_thread {
//             let r = rng;
//             rng = u.run(move |ctx| {
//                 let mut rng = r;
//                 for _ in 0..num_mint_per_update {
//                     rng = rng.wrapping_mul(48271) % 0x7fffffff;
//                     black_box(mint(ctx, rng as u8, 17));
//                 }
//                 rng
//             });
//         }
//
//         num_query_per_thread
//     } else {
//         let mut updates = 0;
//         loop {
//             let r = rng;
//             rng = u.run(move |ctx| {
//                 let mut rng = r;
//                 for _ in 0..num_mint_per_update {
//                     rng = rng.wrapping_mul(48271) % 0x7fffffff;
//                     black_box(mint(ctx, rng as u8, 17));
//                 }
//                 rng
//             });
//             updates += 1;
//             if updates % 10 == 0 && PENDING.load(std::sync::atomic::Ordering::Relaxed) == 0 {
//                 break;
//             }
//         }
//         updates
//     };
//     let updates_duration = now.elapsed();
//
//     // Compute stuff
//     let query_times: Vec<Duration> = handles.into_iter().map(|q| q.join().unwrap()).collect();
//     let total_query_time: Duration = query_times.iter().sum();
//
//     println!("<-UPDATES--------------------------------->");
//     print_report(updates_duration, updates);
//
//     println!("<-TOTAL NUM OF MINTS---------------------->");
//     print_report(updates_duration, updates * num_mint_per_update);
//
//     if num_query_threads > 0 {
//         println!("<-QUERIES [REAL TIME]--------------------->");
//         print_report(updates_duration, num_query_threads * num_query_per_thread);
//     }
//
//     if num_query_threads > 1 {
//         println!("<-QUERIES [CPU TIME]---------------------->");
//         print_report(total_query_time, num_query_threads * num_query_per_thread);
//
//         println!("<-QUERIES [EACH THREAD]------------------->");
//         print_report_vec(&query_times, num_query_per_thread);
//     }
// }
//
// fn print_report(duration: Duration, count: usize) {
//     print_report_vec(&[duration], count);
// }
//
// fn print_report_vec(duration: &[Duration], count: usize) {
//     println!(
//         "  Count = {}",
//         format_vec(duration.iter(), |_| format!("{count}"))
//     );
//     println!(
//         "  Took  = {}",
//         format_vec(duration.iter(), |d| format!("{:.2}s", d.as_secs_f64()))
//     );
//     println!(
//         "  Op/s  = {}",
//         format_vec(duration.iter(), |d| format!("{:.2}", op_per_sec(d, count)))
//     );
//     println!(
//         "  t/Op  = {}",
//         format_vec(duration.iter(), |d| format!(
//             "{:.3}μs",
//             (d.as_micros() as f64) / (count as f64)
//         ))
//     );
// }
//
// fn format_vec<T>(v: impl Iterator<Item = T>, f: impl Fn(&T) -> String) -> String {
//     let mut result = String::new();
//     for item in v {
//         result.push_str(&format!("{: ^12}", f(&item)));
//     }
//     result
// }
//
// fn op_per_sec(duration: &Duration, count: usize) -> usize {
//     let duration_sec = duration.as_secs_f64();
//     ((count as f64) / duration_sec) as usize
// }
//
// fn get_arg(name: &str) -> Option<usize> {
//     let mut args = std::env::args();
//
//     while let Some(arg) = args.next() {
//         if arg == name {
//             let value = args.next().expect("invalid cmd");
//             let number = value.parse::<usize>().expect("invalid arg");
//             return Some(number);
//         }
//     }
//
//     None
// }
//
fn main() {}
