use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RGBConfig {
    pub brightness: u8,
    pub rgb_mode: RGBMode,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RGBMode {
    Cycle,
    Fixed { led1: LedRgb, led2: LedRgb },
    Breathing { led1: LedRgb, led2: LedRgb },
}

/// Three bytes, so it is `Copy`: passing it by value is cheaper than a reference.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedRgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Default for RGBConfig {
    fn default() -> Self {
        Self {
            brightness: 254,
            rgb_mode: RGBMode::Cycle,
        }
    }
}

impl RGBConfig {
    pub fn check_255(&mut self) {
        if self.brightness == u8::MAX {
            self.brightness -= 1;
        }

        self.rgb_mode.check_255();
    }
}

impl RGBMode {
    pub fn check_255(&mut self) {
        match self {
            RGBMode::Cycle => {}
            RGBMode::Fixed { led1, led2 } => {
                led1.check_255();
                led2.check_255();
            }
            RGBMode::Breathing { led1, led2 } => {
                led1.check_255();
                led2.check_255();
            }
        }
    }
}

impl LedRgb {
    pub fn check_255(&mut self) {
        if self.red == u8::MAX {
            self.red -= 1;
        }
        if self.green == u8::MAX {
            self.green -= 1;
        }
        if self.blue == u8::MAX {
            self.blue -= 1;
        }
    }
}
