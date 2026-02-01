use std::time::Instant;

fn main() {
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(143);
    let iterations = 1000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = text.len();
    }
    let elapsed = start.elapsed();

    println!("Iterations: {iterations}");
    println!("Text length: {} bytes", text.len());
    println!("Total time: {elapsed:?}");
    println!("Avg per op: {} ns", elapsed.as_nanos() / iterations as u128);
}
