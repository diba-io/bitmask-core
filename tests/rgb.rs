mod rgb {

    mod unit {
        mod invoice;
        mod issue;
        mod psbt;
        mod stock;
        pub mod utils;
    }

    mod integration {
        mod fungibles;
        mod import;
        mod issue;
        mod udas;
        pub mod utils;
    }

    mod wasm {
        mod std;
    }
}
