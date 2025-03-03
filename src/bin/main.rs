#[tokio::main]
async fn main() {
    
}




// 1. Connect to Hyperliquid and pull (only) the top of book from the orderbook feed.
// 2. Add support for multiple websocket connections to receive the same data, simulating redundant data streams.
// 3. Develop a data structure that allows discarding repeated messages. Limit the data structure to not hold more than 100 items. If, for example, you are using a HashSet, ensure that no more than 100 elements are being stored.
// 4. Use [yawc](https://docs.rs/yawc) for handling websocket connections.
// 5. Print out the latest top of book and log every eviction and addition from the data structure.

// ### Notes
// - Code can be written using tokio tasks or single-thread tasks (aka polling the streams).
// - The data structure should support continuous real-time updates.
// - The program should run indefinitely, maintaining the latest data while discarding duplicates and limiting the total number of stored items