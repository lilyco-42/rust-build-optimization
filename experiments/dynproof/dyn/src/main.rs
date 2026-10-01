// 自动生成 —— 请勿手改。见 gen.py
// 本文件与另一变体逐字节相同，唯一差异是 drive 的签名（静态/动态分发）。

pub trait Work {
    fn step(&self, x: u64) -> u64;
}

pub struct T0;
impl Work for T0 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x85ebca6b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(0_u64);
        a.rotate_left(13)
    }
}

pub struct T1;
impl Work for T1 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x24234424_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(1_u64);
        a.rotate_left(13)
    }
}

pub struct T2;
impl Work for T2 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc25abddd_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(2_u64);
        a.rotate_left(13)
    }
}

pub struct T3;
impl Work for T3 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x60923796_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(3_u64);
        a.rotate_left(13)
    }
}

pub struct T4;
impl Work for T4 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xfec9b14f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(4_u64);
        a.rotate_left(13)
    }
}

pub struct T5;
impl Work for T5 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x9d012b08_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(5_u64);
        a.rotate_left(13)
    }
}

pub struct T6;
impl Work for T6 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3b38a4c1_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(6_u64);
        a.rotate_left(13)
    }
}

pub struct T7;
impl Work for T7 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd9701e7a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(7_u64);
        a.rotate_left(13)
    }
}

pub struct T8;
impl Work for T8 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x77a79833_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(8_u64);
        a.rotate_left(13)
    }
}

pub struct T9;
impl Work for T9 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x15df11ec_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(9_u64);
        a.rotate_left(13)
    }
}

pub struct T10;
impl Work for T10 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb4168ba5_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(10_u64);
        a.rotate_left(13)
    }
}

pub struct T11;
impl Work for T11 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x524e055e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(11_u64);
        a.rotate_left(13)
    }
}

pub struct T12;
impl Work for T12 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf0857f17_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(12_u64);
        a.rotate_left(13)
    }
}

pub struct T13;
impl Work for T13 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8ebcf8d0_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(13_u64);
        a.rotate_left(13)
    }
}

pub struct T14;
impl Work for T14 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x2cf47289_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(14_u64);
        a.rotate_left(13)
    }
}

pub struct T15;
impl Work for T15 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xcb2bec42_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(15_u64);
        a.rotate_left(13)
    }
}

pub struct T16;
impl Work for T16 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x696365fb_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(16_u64);
        a.rotate_left(13)
    }
}

pub struct T17;
impl Work for T17 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x79adfb4_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(17_u64);
        a.rotate_left(13)
    }
}

pub struct T18;
impl Work for T18 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa5d2596d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(18_u64);
        a.rotate_left(13)
    }
}

pub struct T19;
impl Work for T19 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x4409d326_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(19_u64);
        a.rotate_left(13)
    }
}

pub struct T20;
impl Work for T20 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe2414cdf_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(20_u64);
        a.rotate_left(13)
    }
}

pub struct T21;
impl Work for T21 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8078c698_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(21_u64);
        a.rotate_left(13)
    }
}

pub struct T22;
impl Work for T22 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1eb04051_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(22_u64);
        a.rotate_left(13)
    }
}

pub struct T23;
impl Work for T23 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xbce7ba0a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(23_u64);
        a.rotate_left(13)
    }
}

pub struct T24;
impl Work for T24 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x5b1f33c3_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(24_u64);
        a.rotate_left(13)
    }
}

pub struct T25;
impl Work for T25 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf956ad7c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(25_u64);
        a.rotate_left(13)
    }
}

pub struct T26;
impl Work for T26 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x978e2735_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(26_u64);
        a.rotate_left(13)
    }
}

pub struct T27;
impl Work for T27 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x35c5a0ee_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(27_u64);
        a.rotate_left(13)
    }
}

pub struct T28;
impl Work for T28 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd3fd1aa7_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(28_u64);
        a.rotate_left(13)
    }
}

pub struct T29;
impl Work for T29 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x72349460_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(29_u64);
        a.rotate_left(13)
    }
}

pub struct T30;
impl Work for T30 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x106c0e19_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(30_u64);
        a.rotate_left(13)
    }
}

pub struct T31;
impl Work for T31 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xaea387d2_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(31_u64);
        a.rotate_left(13)
    }
}

pub struct T32;
impl Work for T32 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x4cdb018b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(32_u64);
        a.rotate_left(13)
    }
}

pub struct T33;
impl Work for T33 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xeb127b44_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(33_u64);
        a.rotate_left(13)
    }
}

pub struct T34;
impl Work for T34 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8949f4fd_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(34_u64);
        a.rotate_left(13)
    }
}

pub struct T35;
impl Work for T35 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x27816eb6_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(35_u64);
        a.rotate_left(13)
    }
}

pub struct T36;
impl Work for T36 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc5b8e86f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(36_u64);
        a.rotate_left(13)
    }
}

pub struct T37;
impl Work for T37 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x63f06228_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(37_u64);
        a.rotate_left(13)
    }
}

pub struct T38;
impl Work for T38 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x227dbe1_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(38_u64);
        a.rotate_left(13)
    }
}

pub struct T39;
impl Work for T39 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa05f559a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(39_u64);
        a.rotate_left(13)
    }
}

pub struct T40;
impl Work for T40 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3e96cf53_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(40_u64);
        a.rotate_left(13)
    }
}

pub struct T41;
impl Work for T41 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xdcce490c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(41_u64);
        a.rotate_left(13)
    }
}

pub struct T42;
impl Work for T42 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x7b05c2c5_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(42_u64);
        a.rotate_left(13)
    }
}

pub struct T43;
impl Work for T43 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x193d3c7e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(43_u64);
        a.rotate_left(13)
    }
}

pub struct T44;
impl Work for T44 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb774b637_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(44_u64);
        a.rotate_left(13)
    }
}

pub struct T45;
impl Work for T45 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x55ac2ff0_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(45_u64);
        a.rotate_left(13)
    }
}

pub struct T46;
impl Work for T46 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf3e3a9a9_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(46_u64);
        a.rotate_left(13)
    }
}

pub struct T47;
impl Work for T47 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x921b2362_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(47_u64);
        a.rotate_left(13)
    }
}

pub struct T48;
impl Work for T48 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x30529d1b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(48_u64);
        a.rotate_left(13)
    }
}

pub struct T49;
impl Work for T49 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xce8a16d4_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(49_u64);
        a.rotate_left(13)
    }
}

pub struct T50;
impl Work for T50 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6cc1908d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(50_u64);
        a.rotate_left(13)
    }
}

pub struct T51;
impl Work for T51 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xaf90a46_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(51_u64);
        a.rotate_left(13)
    }
}

pub struct T52;
impl Work for T52 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa93083ff_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(52_u64);
        a.rotate_left(13)
    }
}

pub struct T53;
impl Work for T53 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x4767fdb8_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(53_u64);
        a.rotate_left(13)
    }
}

pub struct T54;
impl Work for T54 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe59f7771_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(54_u64);
        a.rotate_left(13)
    }
}

pub struct T55;
impl Work for T55 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x83d6f12a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(55_u64);
        a.rotate_left(13)
    }
}

pub struct T56;
impl Work for T56 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x220e6ae3_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(56_u64);
        a.rotate_left(13)
    }
}

pub struct T57;
impl Work for T57 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc045e49c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(57_u64);
        a.rotate_left(13)
    }
}

pub struct T58;
impl Work for T58 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x5e7d5e55_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(58_u64);
        a.rotate_left(13)
    }
}

pub struct T59;
impl Work for T59 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xfcb4d80e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(59_u64);
        a.rotate_left(13)
    }
}

pub struct T60;
impl Work for T60 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x9aec51c7_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(60_u64);
        a.rotate_left(13)
    }
}

pub struct T61;
impl Work for T61 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3923cb80_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(61_u64);
        a.rotate_left(13)
    }
}

pub struct T62;
impl Work for T62 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd75b4539_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(62_u64);
        a.rotate_left(13)
    }
}

pub struct T63;
impl Work for T63 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x7592bef2_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(63_u64);
        a.rotate_left(13)
    }
}

pub struct T64;
impl Work for T64 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x13ca38ab_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(64_u64);
        a.rotate_left(13)
    }
}

pub struct T65;
impl Work for T65 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb201b264_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(65_u64);
        a.rotate_left(13)
    }
}

pub struct T66;
impl Work for T66 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x50392c1d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(66_u64);
        a.rotate_left(13)
    }
}

pub struct T67;
impl Work for T67 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xee70a5d6_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(67_u64);
        a.rotate_left(13)
    }
}

pub struct T68;
impl Work for T68 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8ca81f8f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(68_u64);
        a.rotate_left(13)
    }
}

pub struct T69;
impl Work for T69 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x2adf9948_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(69_u64);
        a.rotate_left(13)
    }
}

pub struct T70;
impl Work for T70 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc9171301_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(70_u64);
        a.rotate_left(13)
    }
}

pub struct T71;
impl Work for T71 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x674e8cba_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(71_u64);
        a.rotate_left(13)
    }
}

pub struct T72;
impl Work for T72 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x5860673_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(72_u64);
        a.rotate_left(13)
    }
}

pub struct T73;
impl Work for T73 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa3bd802c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(73_u64);
        a.rotate_left(13)
    }
}

pub struct T74;
impl Work for T74 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x41f4f9e5_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(74_u64);
        a.rotate_left(13)
    }
}

pub struct T75;
impl Work for T75 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe02c739e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(75_u64);
        a.rotate_left(13)
    }
}

pub struct T76;
impl Work for T76 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x7e63ed57_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(76_u64);
        a.rotate_left(13)
    }
}

pub struct T77;
impl Work for T77 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1c9b6710_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(77_u64);
        a.rotate_left(13)
    }
}

pub struct T78;
impl Work for T78 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xbad2e0c9_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(78_u64);
        a.rotate_left(13)
    }
}

pub struct T79;
impl Work for T79 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x590a5a82_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(79_u64);
        a.rotate_left(13)
    }
}

pub struct T80;
impl Work for T80 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf741d43b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(80_u64);
        a.rotate_left(13)
    }
}

pub struct T81;
impl Work for T81 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x95794df4_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(81_u64);
        a.rotate_left(13)
    }
}

pub struct T82;
impl Work for T82 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x33b0c7ad_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(82_u64);
        a.rotate_left(13)
    }
}

pub struct T83;
impl Work for T83 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd1e84166_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(83_u64);
        a.rotate_left(13)
    }
}

pub struct T84;
impl Work for T84 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x701fbb1f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(84_u64);
        a.rotate_left(13)
    }
}

pub struct T85;
impl Work for T85 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe5734d8_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(85_u64);
        a.rotate_left(13)
    }
}

pub struct T86;
impl Work for T86 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xac8eae91_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(86_u64);
        a.rotate_left(13)
    }
}

pub struct T87;
impl Work for T87 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x4ac6284a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(87_u64);
        a.rotate_left(13)
    }
}

pub struct T88;
impl Work for T88 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe8fda203_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(88_u64);
        a.rotate_left(13)
    }
}

pub struct T89;
impl Work for T89 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x87351bbc_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(89_u64);
        a.rotate_left(13)
    }
}

pub struct T90;
impl Work for T90 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x256c9575_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(90_u64);
        a.rotate_left(13)
    }
}

pub struct T91;
impl Work for T91 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc3a40f2e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(91_u64);
        a.rotate_left(13)
    }
}

pub struct T92;
impl Work for T92 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x61db88e7_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(92_u64);
        a.rotate_left(13)
    }
}

pub struct T93;
impl Work for T93 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1302a0_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(93_u64);
        a.rotate_left(13)
    }
}

pub struct T94;
impl Work for T94 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x9e4a7c59_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(94_u64);
        a.rotate_left(13)
    }
}

pub struct T95;
impl Work for T95 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3c81f612_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(95_u64);
        a.rotate_left(13)
    }
}

pub struct T96;
impl Work for T96 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xdab96fcb_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(96_u64);
        a.rotate_left(13)
    }
}

pub struct T97;
impl Work for T97 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x78f0e984_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(97_u64);
        a.rotate_left(13)
    }
}

pub struct T98;
impl Work for T98 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1728633d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(98_u64);
        a.rotate_left(13)
    }
}

pub struct T99;
impl Work for T99 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb55fdcf6_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(99_u64);
        a.rotate_left(13)
    }
}

pub struct T100;
impl Work for T100 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x539756af_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(100_u64);
        a.rotate_left(13)
    }
}

pub struct T101;
impl Work for T101 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf1ced068_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(101_u64);
        a.rotate_left(13)
    }
}

pub struct T102;
impl Work for T102 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x90064a21_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(102_u64);
        a.rotate_left(13)
    }
}

pub struct T103;
impl Work for T103 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x2e3dc3da_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(103_u64);
        a.rotate_left(13)
    }
}

pub struct T104;
impl Work for T104 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xcc753d93_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(104_u64);
        a.rotate_left(13)
    }
}

pub struct T105;
impl Work for T105 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6aacb74c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(105_u64);
        a.rotate_left(13)
    }
}

pub struct T106;
impl Work for T106 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8e43105_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(106_u64);
        a.rotate_left(13)
    }
}

pub struct T107;
impl Work for T107 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa71baabe_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(107_u64);
        a.rotate_left(13)
    }
}

pub struct T108;
impl Work for T108 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x45532477_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(108_u64);
        a.rotate_left(13)
    }
}

pub struct T109;
impl Work for T109 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe38a9e30_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(109_u64);
        a.rotate_left(13)
    }
}

pub struct T110;
impl Work for T110 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x81c217e9_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(110_u64);
        a.rotate_left(13)
    }
}

pub struct T111;
impl Work for T111 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1ff991a2_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(111_u64);
        a.rotate_left(13)
    }
}

pub struct T112;
impl Work for T112 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xbe310b5b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(112_u64);
        a.rotate_left(13)
    }
}

pub struct T113;
impl Work for T113 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x5c688514_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(113_u64);
        a.rotate_left(13)
    }
}

pub struct T114;
impl Work for T114 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xfa9ffecd_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(114_u64);
        a.rotate_left(13)
    }
}

pub struct T115;
impl Work for T115 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x98d77886_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(115_u64);
        a.rotate_left(13)
    }
}

pub struct T116;
impl Work for T116 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x370ef23f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(116_u64);
        a.rotate_left(13)
    }
}

pub struct T117;
impl Work for T117 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd5466bf8_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(117_u64);
        a.rotate_left(13)
    }
}

pub struct T118;
impl Work for T118 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x737de5b1_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(118_u64);
        a.rotate_left(13)
    }
}

pub struct T119;
impl Work for T119 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x11b55f6a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(119_u64);
        a.rotate_left(13)
    }
}

pub struct T120;
impl Work for T120 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xafecd923_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(120_u64);
        a.rotate_left(13)
    }
}

pub struct T121;
impl Work for T121 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x4e2452dc_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(121_u64);
        a.rotate_left(13)
    }
}

pub struct T122;
impl Work for T122 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xec5bcc95_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(122_u64);
        a.rotate_left(13)
    }
}

pub struct T123;
impl Work for T123 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8a93464e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(123_u64);
        a.rotate_left(13)
    }
}

pub struct T124;
impl Work for T124 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x28cac007_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(124_u64);
        a.rotate_left(13)
    }
}

pub struct T125;
impl Work for T125 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc70239c0_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(125_u64);
        a.rotate_left(13)
    }
}

pub struct T126;
impl Work for T126 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6539b379_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(126_u64);
        a.rotate_left(13)
    }
}

pub struct T127;
impl Work for T127 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3712d32_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(127_u64);
        a.rotate_left(13)
    }
}

pub struct T128;
impl Work for T128 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa1a8a6eb_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(128_u64);
        a.rotate_left(13)
    }
}

pub struct T129;
impl Work for T129 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3fe020a4_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(129_u64);
        a.rotate_left(13)
    }
}

pub struct T130;
impl Work for T130 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xde179a5d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(130_u64);
        a.rotate_left(13)
    }
}

pub struct T131;
impl Work for T131 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x7c4f1416_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(131_u64);
        a.rotate_left(13)
    }
}

pub struct T132;
impl Work for T132 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1a868dcf_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(132_u64);
        a.rotate_left(13)
    }
}

pub struct T133;
impl Work for T133 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb8be0788_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(133_u64);
        a.rotate_left(13)
    }
}

pub struct T134;
impl Work for T134 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x56f58141_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(134_u64);
        a.rotate_left(13)
    }
}

pub struct T135;
impl Work for T135 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf52cfafa_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(135_u64);
        a.rotate_left(13)
    }
}

pub struct T136;
impl Work for T136 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x936474b3_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(136_u64);
        a.rotate_left(13)
    }
}

pub struct T137;
impl Work for T137 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x319bee6c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(137_u64);
        a.rotate_left(13)
    }
}

pub struct T138;
impl Work for T138 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xcfd36825_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(138_u64);
        a.rotate_left(13)
    }
}

pub struct T139;
impl Work for T139 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6e0ae1de_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(139_u64);
        a.rotate_left(13)
    }
}

pub struct T140;
impl Work for T140 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc425b97_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(140_u64);
        a.rotate_left(13)
    }
}

pub struct T141;
impl Work for T141 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xaa79d550_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(141_u64);
        a.rotate_left(13)
    }
}

pub struct T142;
impl Work for T142 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x48b14f09_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(142_u64);
        a.rotate_left(13)
    }
}

pub struct T143;
impl Work for T143 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe6e8c8c2_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(143_u64);
        a.rotate_left(13)
    }
}

pub struct T144;
impl Work for T144 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8520427b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(144_u64);
        a.rotate_left(13)
    }
}

pub struct T145;
impl Work for T145 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x2357bc34_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(145_u64);
        a.rotate_left(13)
    }
}

pub struct T146;
impl Work for T146 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc18f35ed_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(146_u64);
        a.rotate_left(13)
    }
}

pub struct T147;
impl Work for T147 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x5fc6afa6_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(147_u64);
        a.rotate_left(13)
    }
}

pub struct T148;
impl Work for T148 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xfdfe295f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(148_u64);
        a.rotate_left(13)
    }
}

pub struct T149;
impl Work for T149 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x9c35a318_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(149_u64);
        a.rotate_left(13)
    }
}

pub struct T150;
impl Work for T150 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3a6d1cd1_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(150_u64);
        a.rotate_left(13)
    }
}

pub struct T151;
impl Work for T151 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd8a4968a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(151_u64);
        a.rotate_left(13)
    }
}

pub struct T152;
impl Work for T152 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x76dc1043_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(152_u64);
        a.rotate_left(13)
    }
}

pub struct T153;
impl Work for T153 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x151389fc_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(153_u64);
        a.rotate_left(13)
    }
}

pub struct T154;
impl Work for T154 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb34b03b5_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(154_u64);
        a.rotate_left(13)
    }
}

pub struct T155;
impl Work for T155 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x51827d6e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(155_u64);
        a.rotate_left(13)
    }
}

pub struct T156;
impl Work for T156 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xefb9f727_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(156_u64);
        a.rotate_left(13)
    }
}

pub struct T157;
impl Work for T157 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x8df170e0_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(157_u64);
        a.rotate_left(13)
    }
}

pub struct T158;
impl Work for T158 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x2c28ea99_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(158_u64);
        a.rotate_left(13)
    }
}

pub struct T159;
impl Work for T159 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xca606452_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(159_u64);
        a.rotate_left(13)
    }
}

pub struct T160;
impl Work for T160 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6897de0b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(160_u64);
        a.rotate_left(13)
    }
}

pub struct T161;
impl Work for T161 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6cf57c4_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(161_u64);
        a.rotate_left(13)
    }
}

pub struct T162;
impl Work for T162 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa506d17d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(162_u64);
        a.rotate_left(13)
    }
}

pub struct T163;
impl Work for T163 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x433e4b36_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(163_u64);
        a.rotate_left(13)
    }
}

pub struct T164;
impl Work for T164 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe175c4ef_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(164_u64);
        a.rotate_left(13)
    }
}

pub struct T165;
impl Work for T165 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x7fad3ea8_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(165_u64);
        a.rotate_left(13)
    }
}

pub struct T166;
impl Work for T166 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1de4b861_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(166_u64);
        a.rotate_left(13)
    }
}

pub struct T167;
impl Work for T167 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xbc1c321a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(167_u64);
        a.rotate_left(13)
    }
}

pub struct T168;
impl Work for T168 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x5a53abd3_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(168_u64);
        a.rotate_left(13)
    }
}

pub struct T169;
impl Work for T169 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf88b258c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(169_u64);
        a.rotate_left(13)
    }
}

pub struct T170;
impl Work for T170 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x96c29f45_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(170_u64);
        a.rotate_left(13)
    }
}

pub struct T171;
impl Work for T171 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x34fa18fe_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(171_u64);
        a.rotate_left(13)
    }
}

pub struct T172;
impl Work for T172 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xd33192b7_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(172_u64);
        a.rotate_left(13)
    }
}

pub struct T173;
impl Work for T173 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x71690c70_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(173_u64);
        a.rotate_left(13)
    }
}

pub struct T174;
impl Work for T174 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xfa08629_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(174_u64);
        a.rotate_left(13)
    }
}

pub struct T175;
impl Work for T175 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xadd7ffe2_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(175_u64);
        a.rotate_left(13)
    }
}

pub struct T176;
impl Work for T176 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x4c0f799b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(176_u64);
        a.rotate_left(13)
    }
}

pub struct T177;
impl Work for T177 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xea46f354_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(177_u64);
        a.rotate_left(13)
    }
}

pub struct T178;
impl Work for T178 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x887e6d0d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(178_u64);
        a.rotate_left(13)
    }
}

pub struct T179;
impl Work for T179 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x26b5e6c6_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(179_u64);
        a.rotate_left(13)
    }
}

pub struct T180;
impl Work for T180 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xc4ed607f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(180_u64);
        a.rotate_left(13)
    }
}

pub struct T181;
impl Work for T181 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6324da38_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(181_u64);
        a.rotate_left(13)
    }
}

pub struct T182;
impl Work for T182 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x15c53f1_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(182_u64);
        a.rotate_left(13)
    }
}

pub struct T183;
impl Work for T183 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x9f93cdaa_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(183_u64);
        a.rotate_left(13)
    }
}

pub struct T184;
impl Work for T184 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x3dcb4763_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(184_u64);
        a.rotate_left(13)
    }
}

pub struct T185;
impl Work for T185 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xdc02c11c_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(185_u64);
        a.rotate_left(13)
    }
}

pub struct T186;
impl Work for T186 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x7a3a3ad5_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(186_u64);
        a.rotate_left(13)
    }
}

pub struct T187;
impl Work for T187 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x1871b48e_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(187_u64);
        a.rotate_left(13)
    }
}

pub struct T188;
impl Work for T188 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xb6a92e47_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(188_u64);
        a.rotate_left(13)
    }
}

pub struct T189;
impl Work for T189 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x54e0a800_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(189_u64);
        a.rotate_left(13)
    }
}

pub struct T190;
impl Work for T190 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xf31821b9_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(190_u64);
        a.rotate_left(13)
    }
}

pub struct T191;
impl Work for T191 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x914f9b72_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(191_u64);
        a.rotate_left(13)
    }
}

pub struct T192;
impl Work for T192 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x2f87152b_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(192_u64);
        a.rotate_left(13)
    }
}

pub struct T193;
impl Work for T193 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xcdbe8ee4_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(193_u64);
        a.rotate_left(13)
    }
}

pub struct T194;
impl Work for T194 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x6bf6089d_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(194_u64);
        a.rotate_left(13)
    }
}

pub struct T195;
impl Work for T195 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa2d8256_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(195_u64);
        a.rotate_left(13)
    }
}

pub struct T196;
impl Work for T196 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xa864fc0f_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(196_u64);
        a.rotate_left(13)
    }
}

pub struct T197;
impl Work for T197 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x469c75c8_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(197_u64);
        a.rotate_left(13)
    }
}

pub struct T198;
impl Work for T198 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0xe4d3ef81_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(198_u64);
        a.rotate_left(13)
    }
}

pub struct T199;
impl Work for T199 {
    fn step(&self, x: u64) -> u64 {
        let mut a = x ^ 0x830b693a_u64;
        a = a.wrapping_mul(0x100000001B3);
        a ^= a >> 29;
        a = a.wrapping_add(199_u64);
        a.rotate_left(13)
    }
}

pub fn drive(t: &dyn Work, n: u64) -> u64 {

    let mut acc = 0u64;
    for i in 0..n {
        acc = acc.wrapping_mul(31).wrapping_add(t.step(acc ^ i));
        acc ^= acc >> 13;
        if acc % 7 == 3 {
            acc = acc.rotate_left(5);
        }
        acc = acc.wrapping_add(i & 0xFF);
        if acc & 1 == 0 {
            acc ^= acc >> 17;
        } else {
            acc = acc.wrapping_mul(3);
        }
    }
    acc

}

fn main() {
    let mut total = 0u64;
    total = total.wrapping_add(drive(&T0, 200));
    total = total.wrapping_add(drive(&T1, 200));
    total = total.wrapping_add(drive(&T2, 200));
    total = total.wrapping_add(drive(&T3, 200));
    total = total.wrapping_add(drive(&T4, 200));
    total = total.wrapping_add(drive(&T5, 200));
    total = total.wrapping_add(drive(&T6, 200));
    total = total.wrapping_add(drive(&T7, 200));
    total = total.wrapping_add(drive(&T8, 200));
    total = total.wrapping_add(drive(&T9, 200));
    total = total.wrapping_add(drive(&T10, 200));
    total = total.wrapping_add(drive(&T11, 200));
    total = total.wrapping_add(drive(&T12, 200));
    total = total.wrapping_add(drive(&T13, 200));
    total = total.wrapping_add(drive(&T14, 200));
    total = total.wrapping_add(drive(&T15, 200));
    total = total.wrapping_add(drive(&T16, 200));
    total = total.wrapping_add(drive(&T17, 200));
    total = total.wrapping_add(drive(&T18, 200));
    total = total.wrapping_add(drive(&T19, 200));
    total = total.wrapping_add(drive(&T20, 200));
    total = total.wrapping_add(drive(&T21, 200));
    total = total.wrapping_add(drive(&T22, 200));
    total = total.wrapping_add(drive(&T23, 200));
    total = total.wrapping_add(drive(&T24, 200));
    total = total.wrapping_add(drive(&T25, 200));
    total = total.wrapping_add(drive(&T26, 200));
    total = total.wrapping_add(drive(&T27, 200));
    total = total.wrapping_add(drive(&T28, 200));
    total = total.wrapping_add(drive(&T29, 200));
    total = total.wrapping_add(drive(&T30, 200));
    total = total.wrapping_add(drive(&T31, 200));
    total = total.wrapping_add(drive(&T32, 200));
    total = total.wrapping_add(drive(&T33, 200));
    total = total.wrapping_add(drive(&T34, 200));
    total = total.wrapping_add(drive(&T35, 200));
    total = total.wrapping_add(drive(&T36, 200));
    total = total.wrapping_add(drive(&T37, 200));
    total = total.wrapping_add(drive(&T38, 200));
    total = total.wrapping_add(drive(&T39, 200));
    total = total.wrapping_add(drive(&T40, 200));
    total = total.wrapping_add(drive(&T41, 200));
    total = total.wrapping_add(drive(&T42, 200));
    total = total.wrapping_add(drive(&T43, 200));
    total = total.wrapping_add(drive(&T44, 200));
    total = total.wrapping_add(drive(&T45, 200));
    total = total.wrapping_add(drive(&T46, 200));
    total = total.wrapping_add(drive(&T47, 200));
    total = total.wrapping_add(drive(&T48, 200));
    total = total.wrapping_add(drive(&T49, 200));
    total = total.wrapping_add(drive(&T50, 200));
    total = total.wrapping_add(drive(&T51, 200));
    total = total.wrapping_add(drive(&T52, 200));
    total = total.wrapping_add(drive(&T53, 200));
    total = total.wrapping_add(drive(&T54, 200));
    total = total.wrapping_add(drive(&T55, 200));
    total = total.wrapping_add(drive(&T56, 200));
    total = total.wrapping_add(drive(&T57, 200));
    total = total.wrapping_add(drive(&T58, 200));
    total = total.wrapping_add(drive(&T59, 200));
    total = total.wrapping_add(drive(&T60, 200));
    total = total.wrapping_add(drive(&T61, 200));
    total = total.wrapping_add(drive(&T62, 200));
    total = total.wrapping_add(drive(&T63, 200));
    total = total.wrapping_add(drive(&T64, 200));
    total = total.wrapping_add(drive(&T65, 200));
    total = total.wrapping_add(drive(&T66, 200));
    total = total.wrapping_add(drive(&T67, 200));
    total = total.wrapping_add(drive(&T68, 200));
    total = total.wrapping_add(drive(&T69, 200));
    total = total.wrapping_add(drive(&T70, 200));
    total = total.wrapping_add(drive(&T71, 200));
    total = total.wrapping_add(drive(&T72, 200));
    total = total.wrapping_add(drive(&T73, 200));
    total = total.wrapping_add(drive(&T74, 200));
    total = total.wrapping_add(drive(&T75, 200));
    total = total.wrapping_add(drive(&T76, 200));
    total = total.wrapping_add(drive(&T77, 200));
    total = total.wrapping_add(drive(&T78, 200));
    total = total.wrapping_add(drive(&T79, 200));
    total = total.wrapping_add(drive(&T80, 200));
    total = total.wrapping_add(drive(&T81, 200));
    total = total.wrapping_add(drive(&T82, 200));
    total = total.wrapping_add(drive(&T83, 200));
    total = total.wrapping_add(drive(&T84, 200));
    total = total.wrapping_add(drive(&T85, 200));
    total = total.wrapping_add(drive(&T86, 200));
    total = total.wrapping_add(drive(&T87, 200));
    total = total.wrapping_add(drive(&T88, 200));
    total = total.wrapping_add(drive(&T89, 200));
    total = total.wrapping_add(drive(&T90, 200));
    total = total.wrapping_add(drive(&T91, 200));
    total = total.wrapping_add(drive(&T92, 200));
    total = total.wrapping_add(drive(&T93, 200));
    total = total.wrapping_add(drive(&T94, 200));
    total = total.wrapping_add(drive(&T95, 200));
    total = total.wrapping_add(drive(&T96, 200));
    total = total.wrapping_add(drive(&T97, 200));
    total = total.wrapping_add(drive(&T98, 200));
    total = total.wrapping_add(drive(&T99, 200));
    total = total.wrapping_add(drive(&T100, 200));
    total = total.wrapping_add(drive(&T101, 200));
    total = total.wrapping_add(drive(&T102, 200));
    total = total.wrapping_add(drive(&T103, 200));
    total = total.wrapping_add(drive(&T104, 200));
    total = total.wrapping_add(drive(&T105, 200));
    total = total.wrapping_add(drive(&T106, 200));
    total = total.wrapping_add(drive(&T107, 200));
    total = total.wrapping_add(drive(&T108, 200));
    total = total.wrapping_add(drive(&T109, 200));
    total = total.wrapping_add(drive(&T110, 200));
    total = total.wrapping_add(drive(&T111, 200));
    total = total.wrapping_add(drive(&T112, 200));
    total = total.wrapping_add(drive(&T113, 200));
    total = total.wrapping_add(drive(&T114, 200));
    total = total.wrapping_add(drive(&T115, 200));
    total = total.wrapping_add(drive(&T116, 200));
    total = total.wrapping_add(drive(&T117, 200));
    total = total.wrapping_add(drive(&T118, 200));
    total = total.wrapping_add(drive(&T119, 200));
    total = total.wrapping_add(drive(&T120, 200));
    total = total.wrapping_add(drive(&T121, 200));
    total = total.wrapping_add(drive(&T122, 200));
    total = total.wrapping_add(drive(&T123, 200));
    total = total.wrapping_add(drive(&T124, 200));
    total = total.wrapping_add(drive(&T125, 200));
    total = total.wrapping_add(drive(&T126, 200));
    total = total.wrapping_add(drive(&T127, 200));
    total = total.wrapping_add(drive(&T128, 200));
    total = total.wrapping_add(drive(&T129, 200));
    total = total.wrapping_add(drive(&T130, 200));
    total = total.wrapping_add(drive(&T131, 200));
    total = total.wrapping_add(drive(&T132, 200));
    total = total.wrapping_add(drive(&T133, 200));
    total = total.wrapping_add(drive(&T134, 200));
    total = total.wrapping_add(drive(&T135, 200));
    total = total.wrapping_add(drive(&T136, 200));
    total = total.wrapping_add(drive(&T137, 200));
    total = total.wrapping_add(drive(&T138, 200));
    total = total.wrapping_add(drive(&T139, 200));
    total = total.wrapping_add(drive(&T140, 200));
    total = total.wrapping_add(drive(&T141, 200));
    total = total.wrapping_add(drive(&T142, 200));
    total = total.wrapping_add(drive(&T143, 200));
    total = total.wrapping_add(drive(&T144, 200));
    total = total.wrapping_add(drive(&T145, 200));
    total = total.wrapping_add(drive(&T146, 200));
    total = total.wrapping_add(drive(&T147, 200));
    total = total.wrapping_add(drive(&T148, 200));
    total = total.wrapping_add(drive(&T149, 200));
    total = total.wrapping_add(drive(&T150, 200));
    total = total.wrapping_add(drive(&T151, 200));
    total = total.wrapping_add(drive(&T152, 200));
    total = total.wrapping_add(drive(&T153, 200));
    total = total.wrapping_add(drive(&T154, 200));
    total = total.wrapping_add(drive(&T155, 200));
    total = total.wrapping_add(drive(&T156, 200));
    total = total.wrapping_add(drive(&T157, 200));
    total = total.wrapping_add(drive(&T158, 200));
    total = total.wrapping_add(drive(&T159, 200));
    total = total.wrapping_add(drive(&T160, 200));
    total = total.wrapping_add(drive(&T161, 200));
    total = total.wrapping_add(drive(&T162, 200));
    total = total.wrapping_add(drive(&T163, 200));
    total = total.wrapping_add(drive(&T164, 200));
    total = total.wrapping_add(drive(&T165, 200));
    total = total.wrapping_add(drive(&T166, 200));
    total = total.wrapping_add(drive(&T167, 200));
    total = total.wrapping_add(drive(&T168, 200));
    total = total.wrapping_add(drive(&T169, 200));
    total = total.wrapping_add(drive(&T170, 200));
    total = total.wrapping_add(drive(&T171, 200));
    total = total.wrapping_add(drive(&T172, 200));
    total = total.wrapping_add(drive(&T173, 200));
    total = total.wrapping_add(drive(&T174, 200));
    total = total.wrapping_add(drive(&T175, 200));
    total = total.wrapping_add(drive(&T176, 200));
    total = total.wrapping_add(drive(&T177, 200));
    total = total.wrapping_add(drive(&T178, 200));
    total = total.wrapping_add(drive(&T179, 200));
    total = total.wrapping_add(drive(&T180, 200));
    total = total.wrapping_add(drive(&T181, 200));
    total = total.wrapping_add(drive(&T182, 200));
    total = total.wrapping_add(drive(&T183, 200));
    total = total.wrapping_add(drive(&T184, 200));
    total = total.wrapping_add(drive(&T185, 200));
    total = total.wrapping_add(drive(&T186, 200));
    total = total.wrapping_add(drive(&T187, 200));
    total = total.wrapping_add(drive(&T188, 200));
    total = total.wrapping_add(drive(&T189, 200));
    total = total.wrapping_add(drive(&T190, 200));
    total = total.wrapping_add(drive(&T191, 200));
    total = total.wrapping_add(drive(&T192, 200));
    total = total.wrapping_add(drive(&T193, 200));
    total = total.wrapping_add(drive(&T194, 200));
    total = total.wrapping_add(drive(&T195, 200));
    total = total.wrapping_add(drive(&T196, 200));
    total = total.wrapping_add(drive(&T197, 200));
    total = total.wrapping_add(drive(&T198, 200));
    total = total.wrapping_add(drive(&T199, 200));
    println!("{total}");
}
