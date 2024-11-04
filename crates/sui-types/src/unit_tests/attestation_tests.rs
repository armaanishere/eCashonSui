// // Copyright (c) Mysten Labs, Inc.
// // SPDX-License-Identifier: Apache-2.0

// use super::attestation_verify_inner;
// use fastcrypto::encoding::Encoding;
// use fastcrypto::encoding::Hex;

// #[test]
// fn attestation_parse() {
//     let res = attestation_verify_inner(
//         &Hex::decode("8444a1013822a059111fa9696d6f64756c655f69647827692d30663733613462346362373463633966322d656e633031393265343138386665663738316466646967657374665348413338346974696d657374616d701b00000192e9b6ea646470637273b0005830000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000015830000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000025830000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000035830639a8b65f68b0223cbb14a0032487e5656d260434e3d1a10e7ec1407fb86143860717fc8afee90df7a1604111709af460458309ab5a1aba055ee41ee254b9b251a58259b29fa1096859762744e9ac73b5869b25e51223854d9f86adbb37fe69f3e5d1c0558300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000658300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000758300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000858300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000958300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a58300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b58300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c58300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000d58300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e58300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f58300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006b636572746966696361746559027f3082027b30820201a00302010202100192e4188fef781d0000000067254d8b300a06082a8648ce3d04030330818e310b30090603550406130255533113301106035504080c0a57617368696e67746f6e3110300e06035504070c0753656174746c65310f300d060355040a0c06416d617a6f6e310c300a060355040b0c034157533139303706035504030c30692d30663733613462346362373463633966322e75732d656173742d312e6177732e6e6974726f2d656e636c61766573301e170d3234313130313231353230385a170d3234313130323030353231315a308193310b30090603550406130255533113301106035504080c0a57617368696e67746f6e3110300e06035504070c0753656174746c65310f300d060355040a0c06416d617a6f6e310c300a060355040b0c03415753313e303c06035504030c35692d30663733613462346362373463633966322d656e63303139326534313838666566373831642e75732d656173742d312e6177733076301006072a8648ce3d020106052b81040022036200040e09b2f4b324b664d4406da5ea887ba3939b240214792c00407ba018d3871748387e0026ea6d14eb97dade97529f21ece2c29112e76a559b4f959a4c32ea250fbb43ea093f496a494bd5a11b34155860770bbae28c1304bbac1be4ab6a3018c6a31d301b300c0603551d130101ff04023000300b0603551d0f0404030206c0300a06082a8648ce3d0403030368003065023100abb61bba7ed00fd9b09ec27cd137e52eb09754694ed4a4a75531edff682cf89c8775e417bd656b21e4bdfc75af95ba1602302314f07d90c2a8cc24878aa404d17ee3fd8a8937b0e412e8505e29b69e6ba6dc65561c757e9aa77d9e8236b34cf87bf568636162756e646c65845902153082021130820196a003020102021100f93175681b90afe11d46ccb4e4e7f856300a06082a8648ce3d0403033049310b3009060355040613025553310f300d060355040a0c06416d617a6f6e310c300a060355040b0c03415753311b301906035504030c126177732e6e6974726f2d656e636c61766573301e170d3139313032383133323830355a170d3439313032383134323830355a3049310b3009060355040613025553310f300d060355040a0c06416d617a6f6e310c300a060355040b0c03415753311b301906035504030c126177732e6e6974726f2d656e636c617665733076301006072a8648ce3d020106052b8104002203620004fc0254eba608c1f36870e29ada90be46383292736e894bfff672d989444b5051e534a4b1f6dbe3c0bc581a32b7b176070ede12d69a3fea211b66e752cf7dd1dd095f6f1370f4170843d9dc100121e4cf63012809664487c9796284304dc53ff4a3423040300f0603551d130101ff040530030101ff301d0603551d0e041604149025b50dd90547e796c396fa729dcf99a9df4b96300e0603551d0f0101ff040403020186300a06082a8648ce3d0403030369003066023100a37f2f91a1c9bd5ee7b8627c1698d255038e1f0343f95b63a9628c3d39809545a11ebcbf2e3b55d8aeee71b4c3d6adf3023100a2f39b1605b27028a5dd4ba069b5016e65b4fbde8fe0061d6a53197f9cdaf5d943bc61fc2beb03cb6fee8d2302f3dff65902c1308202bd30820244a00302010202105e38d80df50697821a75868f41e784b0300a06082a8648ce3d0403033049310b3009060355040613025553310f300d060355040a0c06416d617a6f6e310c300a060355040b0c03415753311b301906035504030c126177732e6e6974726f2d656e636c61766573301e170d3234313032393039303734355a170d3234313131383130303734355a3064310b3009060355040613025553310f300d060355040a0c06416d617a6f6e310c300a060355040b0c034157533136303406035504030c2d616662353333343262366335383838352e75732d656173742d312e6177732e6e6974726f2d656e636c617665733076301006072a8648ce3d020106052b8104002203620004c92eff4f5268be6b0b2d5e402610285410046d5ff1ccbbada2cafac96ca22525a89d710bc69c6dddf1c5dde1bb29293416611718f47631c5c61945d82ad49c41a553af78f409763490a3f888edf8188fee6d6691aeba6faf3a559d1086f7437fa381d53081d230120603551d130101ff040830060101ff020102301f0603551d230418301680149025b50dd90547e796c396fa729dcf99a9df4b96301d0603551d0e041604144cb4eb898e87a14e9b196cc57923aa7899c45454300e0603551d0f0101ff040403020186306c0603551d1f046530633061a05fa05d865b687474703a2f2f6177732d6e6974726f2d656e636c617665732d63726c2e73332e616d617a6f6e6177732e636f6d2f63726c2f61623439363063632d376436332d343262642d396539662d3539333338636236376638342e63726c300a06082a8648ce3d0403030367003064023064ff9e5be5f0028e5cd81103213d49839330fcf2110d9e53bb4149fd71e86576ac27c53e886a92081bef34a006b2bead02300a6a31d99fe753f0b582f1ee81975e7c81bfb7a31bac3f41f0456107cb0c4442bfb644f8c672b322fd415b5445021210590318308203143082029aa0030201020210656eddc68ca271e0d87e4556975c6e4a300a06082a8648ce3d0403033064310b3009060355040613025553310f300d060355040a0c06416d617a6f6e310c300a060355040b0c034157533136303406035504030c2d616662353333343262366335383838352e75732d656173742d312e6177732e6e6974726f2d656e636c61766573301e170d3234313130313035353131315a170d3234313130363230353131305a308189313c303a06035504030c33343431653935346366616137656561382e7a6f6e616c2e75732d656173742d312e6177732e6e6974726f2d656e636c61766573310c300a060355040b0c03415753310f300d060355040a0c06416d617a6f6e310b3009060355040613025553310b300906035504080c0257413110300e06035504070c0753656174746c653076301006072a8648ce3d020106052b81040022036200040b7dbaa561f8294f9fba1727c2ecc0bab4abb27d2e04c6a255a4f9ad3e7471e34e32f630e6228552ba684289b6e9ba065484f02fdd29b802f150ad4f3cb3beb5032a96126d223f34ae9430a5152bbcfd4eb1665c6c3e19bbbe8688c8b77900a8a381ea3081e730120603551d130101ff040830060101ff020101301f0603551d230418301680144cb4eb898e87a14e9b196cc57923aa7899c45454301d0603551d0e0416041457eb5d610ec8aeae89028cabb14bdf3672b0ff07300e0603551d0f0101ff0404030201863081800603551d1f047930773075a073a071866f687474703a2f2f63726c2d75732d656173742d312d6177732d6e6974726f2d656e636c617665732e73332e75732d656173742d312e616d617a6f6e6177732e636f6d2f63726c2f66316139303335362d616130622d343934332d613866612d3933613737623036396361332e63726c300a06082a8648ce3d040303036800306502310091f43592545b55ae03ae0dd8233541cf9d21f649a7037e5ff5d09ed3fb459a036727ee07550c9424a6b67040327433c3023001e650662d8435ef39a7c982585b791fa61776d168ebe6295899d68c0d6f19f3e3a1b694f7ca6de6113c51f734627c245902c3308202bf30820245a003020102021500fe7e5aff7f6277a3c929ad170a5c6362fc1d673d300a06082a8648ce3d040303308189313c303a06035504030c33343431653935346366616137656561382e7a6f6e616c2e75732d656173742d312e6177732e6e6974726f2d656e636c61766573310c300a060355040b0c03415753310f300d060355040a0c06416d617a6f6e310b3009060355040613025553310b300906035504080c0257413110300e06035504070c0753656174746c65301e170d3234313130313130313030395a170d3234313130323130313030395a30818e310b30090603550406130255533113301106035504080c0a57617368696e67746f6e3110300e06035504070c0753656174746c65310f300d060355040a0c06416d617a6f6e310c300a060355040b0c034157533139303706035504030c30692d30663733613462346362373463633966322e75732d656173742d312e6177732e6e6974726f2d656e636c617665733076301006072a8648ce3d020106052b81040022036200040fe46adf864a558a00a9ca4b64ece5ba124ed1d29656a1f16ca71d0dc8fca56b0fb15aafd309f6258374e8c7b4a5b0521c76d1812a7873474dae9322aef1cd782db19fc2ece4d36fa08acbe65e4bec2a3cfe70960d179778ea7e7711f827b36ea366306430120603551d130101ff040830060101ff020100300e0603551d0f0101ff040403020204301d0603551d0e041604143e40d423bf86e9565c378487843389bd2f471a56301f0603551d2304183016801457eb5d610ec8aeae89028cabb14bdf3672b0ff07300a06082a8648ce3d0403030368003065023100a3a08f56bc0c28b93d7e13258e497b9ea3245a4c9a56c7271cf545a86d0b09482ceecc7fdb1e5bcdc739def92a4ed8cd02303c384850b6188959b9f1114253376464573563d77e1c6812b1998bf960da478b0cea959f34a8201bf779a8ae615f4d266a7075626c69635f6b6579f669757365725f6461746158205a264748a62368075d34b9494634a3e096e0e48f6647f965b81d2a653de684f2656e6f6e6365f658605700fe1b1ea59f7979bcdd5671b6f478e067b1e54b262b1ea1d078b5b964061e3cd93a952cb14abcf7bdbaf6fff713b9714616948d0e842e5f63a56af9f2cf3594447a140e9eaf429f2320b6082b39c96ad4144b707bb4e0223173221d5d9c7e").unwrap(),
//         &Hex::decode("5a264748a62368075d34b9494634a3e096e0e48f6647f965b81d2a653de684f2").unwrap(),
//         &Hex::decode("000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap(),
//         &Hex::decode("000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap(),
//         &Hex::decode("000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap(),
//     );
//     println!("{:?}", res);
//     assert!(res.is_ok());
// }
