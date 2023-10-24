mod rgb {

    mod unit {
        mod invoice;
        mod issue;
        mod psbt;
        mod stl;
        mod stock;
        pub mod utils;
    }

    mod integration {
        // TODO: Review after support multi-token transfer
        // mod collectibles;
        mod accept;
        mod batch;
        mod cambria;
        mod collectibles;
        mod crdt;
        mod drain;
        mod dustless;
        mod fungibles;
        mod import;
        mod issue;
        mod proxy;
        mod rbf;
        mod states;
        mod swaps;
        mod transfers;
        mod udas;
        pub mod utils;
        mod watcher;
    }

    mod web {
        // mod contracts;
        // mod imports;
        mod proxy;
        // mod stl_ids;
        // mod stl_load;
        // mod swaps;
        // mod transfers;
        mod utils;
    }

    mod sre {
        mod st160;
        mod st160_web;
    }
}
