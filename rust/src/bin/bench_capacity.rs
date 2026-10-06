//! Independent fixed work: no TT, queue, allocation, or alpha-beta overhead.
//! Throughput scaling distinguishes host capacity from search scheduling.
use dosello_ai::board::{perft, Position};
fn main() {
    let threads = std::env::args()
        .nth(1)
        .unwrap_or("1".into())
        .parse::<usize>()
        .unwrap()
        .clamp(1, 64);
    let start = std::time::Instant::now();
    let leaves = std::thread::scope(|scope| {
        let jobs = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    #[cfg(target_os = "macos")]
                    if std::env::var("DOSELLO_QOS").as_deref() != Ok("0") {
                        extern "C" {
                            fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
                        }
                        // SAFETY: only this temporary benchmark worker is changed.
                        unsafe {
                            pthread_set_qos_class_self_np(0x19, 0);
                        }
                    }
                    (0..10)
                        .map(|_| perft(std::hint::black_box(Position::initial()), 6))
                        .sum::<u64>()
                })
            })
            .collect::<Vec<_>>();
        jobs.into_iter().map(|j| j.join().unwrap()).sum::<u64>()
    });
    assert_eq!(leaves, 11619262 * 10 * threads as u64);
    println!(
        "{{\"threads\":{threads},\"leaves\":{leaves},\"seconds\":{}}}",
        start.elapsed().as_secs_f64()
    );
}
