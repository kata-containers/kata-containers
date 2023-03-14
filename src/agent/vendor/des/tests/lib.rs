//! Test vectors are from NESSIE:
//! https://www.cosic.esat.kuleuven.be/nessie/testvectors/

cipher::new_test!(des_test, "des", des::Des);
cipher::new_test!(tdes_ede3_test, "tdes", des::TdesEde3);
cipher::new_test!(tdes_ede2_test, "tdes2", des::TdesEde2);
