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
        mod collectibles;

        mod dustless;
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
        mod stl_ids;
        mod stl_load;
    }

    mod sre {
        mod st150;
        mod st150_web;
    }
}
