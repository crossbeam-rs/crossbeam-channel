cargo run --release --bin chan | tee small_chan.txt
cargo run --release --bin mpsc | tee small_mpsc.txt
cargo run --release --bin crossbeam-channel | tee small_crossbeam-channel.txt
cargo run --release --bin ms_queue | tee small_ms_queue.txt
cargo run --release --bin seg_queue | tee small_seg_queue.txt
cargo run --release --bin mpmc | tee small_mpmc.txt
cargo run --release --bin atomicring | tee small_atomicring.txt
cargo run --release --bin atomicringqueue | tee small_atomicringqueue.txt
go run main.go | tee small_go.txt
./plot.py small_*.txt 
mv plot.png plot_small.png

cargo run --features medium_size --release --bin chan | tee medium_chan.txt
cargo run --features medium_size --release --bin mpsc | tee medium_mpsc.txt
cargo run --features medium_size --release --bin crossbeam-channel | tee medium_crossbeam-channel.txt
cargo run --features medium_size --release --bin ms_queue | tee medium_ms_queue.txt
cargo run --features medium_size --release --bin seg_queue | tee medium_seg_queue.txt
cargo run --features medium_size --release --bin mpmc | tee medium_mpmc.txt
cargo run --features medium_size --release --bin atomicring | tee medium_atomicring.txt
cargo run --features medium_size --release --bin atomicringqueue | tee medium_atomicringqueue.txt
./plot.py medium_*.txt 
mv plot.png plot_medium.png


cargo run --features large_size --release --bin chan | tee large_chan.txt
cargo run --features large_size --release --bin mpsc | tee large_mpsc.txt
cargo run --features large_size --release --bin crossbeam-channel | tee large_crossbeam-channel.txt
cargo run --features large_size --release --bin ms_queue | tee large_ms_queue.txt
cargo run --features large_size --release --bin seg_queue | tee large_seg_queue.txt
cargo run --features large_size --release --bin mpmc | tee large_mpmc.txt
cargo run --features large_size --release --bin atomicring | tee large_atomicring.txt
cargo run --features large_size --release --bin atomicringqueue | tee large_atomicringqueue.txt
./plot.py large_*.txt 
mv plot.png plot_large.png
