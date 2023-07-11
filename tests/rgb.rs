mod rgb {

    mod unit {
        mod invoice;
        mod issue;
        mod psbt;
        mod schemas;
        mod stock;
        pub mod utils;
    }

    mod integration {
        // TODO: Review after support multi-token transfer
        // mod collectibles;
        mod collectibles;

        mod fungibles;
        mod import;
        mod issue;
        mod states;
        mod stress;
        mod transfers;
        mod udas;
        pub mod utils;
        mod watcher;
    }

    mod web {
        mod contracts;
        mod imports;
        mod schemas;
        mod std;
    }

    mod sre {
        mod st140;
        mod st140_web;
    }
}
