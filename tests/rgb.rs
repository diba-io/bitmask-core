mod rgb {

    mod unit {
        mod invoice;
        mod issue;
        mod psbt;
        mod stock;
        pub mod utils;
    }

    mod integration {
        // TODO: Review after support multi-token transfer
        // mod collectibles;
        mod collectibles;

        mod import;
        mod issue;
        // TODO: Uncomment after fixing slow encoding/decoding of strict type
        // mod fungibles;
        // mod states;
        // mod stress;
        // mod transfers;
        // mod udas;
        pub mod utils;
        mod watcher;
    }

    mod web {
        mod contracts;
        mod imports;
        mod std;
    }

    mod sre {
        mod st140;
        mod st140_web;
    }
}
