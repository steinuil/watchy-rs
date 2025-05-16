#![allow(dead_code)]

/// Chip identification code
pub const CHIP_ID: u8 = 0x00;

/// Reports sensor error conditions
pub const ERR_REG: u8 = 0x02;

/// Sensor status flags
pub const STATUS: u8 = 0x03;

/// AUX_X(LSB)
pub const DATA_0: u8 = 0x0A;
/// AUX_X(MSB)
pub const DATA_1: u8 = 0x0B;
/// AUX_Y(LSB)
pub const DATA_2: u8 = 0x0C;
/// AUX_Y(MSB)
pub const DATA_3: u8 = 0x0D;
/// AUX_Z(LSB)
pub const DATA_4: u8 = 0x0E;
/// AUX_Z(MSB)
pub const DATA_5: u8 = 0x0F;
/// AUX_R(LSB)
pub const DATA_6: u8 = 0x10;
/// AUX_R(MSB)
pub const DATA_7: u8 = 0x11;
/// ACC_X(LSB)
pub const DATA_8: u8 = 0x12;
/// ACC_X(MSB)
pub const DATA_9: u8 = 0x13;
/// ACC_Y(LSB)
pub const DATA_10: u8 = 0x14;
/// ACC_Y(MSB)
pub const DATA_11: u8 = 0x15;
/// ACC_Z(LSB)
pub const DATA_12: u8 = 0x16;
/// ACC_Z(MSB)
pub const DATA_13: u8 = 0x17;

/// Sensor time <7:0>
pub const SENSORTIME_0: u8 = 0x18;
/// Sensor time <15:8>
pub const SENSORTIME_1: u8 = 0x19;
/// Sensor time <23:16>
pub const SENSORTIME_2: u8 = 0x1A;

/// Sensor status flags
pub const EVENT: u8 = 0x1B;

/// Interrupt/Feature status. Will be cleared on read.
pub const INT_STATUS_0: u8 = 0x1C;
/// Interrupt status. Will be cleared on read.
pub const INT_STATUS_1: u8 = 0x1D;

/// Step counting value byte-0
pub const STEP_COUNTER_0: u8 = 0x1E;
/// Step counting value byte-1
pub const STEP_COUNTER_1: u8 = 0x1F;
/// Step counting value byte-2
pub const STEP_COUNTER_2: u8 = 0x20;
/// Step counting value byte-3
pub const STEP_COUNTER_3: u8 = 0x21;

/// Contains the temperature value of the sensor
pub const TEMPERATURE: u8 = 0x22;

/// FIFO byte count register (LSB)
pub const FIFO_LENGTH_0: u8 = 0x24;
/// FIFO byte count register (MSB)
pub const FIFO_LENGTH_1: u8 = 0x25;

/// FIFO data output register
pub const FIFO_DATA: u8 = 0x26;

/// Step counter activity output (Running, Walking, Still)
pub const ACTIVITY_TYPE: u8 = 0x27;

/// Error bits and message indicating internal status
pub const INTERNAL_STATUS: u8 = 0x2A;

/// Sets the output data rate, the bandwidth, and the read mode of the acceleration sensor
pub const ACC_CONF: u8 = 0x40;

/// Selection of the Accelerometer g-range
pub const ACC_RANGE: u8 = 0x41;

/// Sets the output data of the Auxillary interface
pub const AUX_CONF: u8 = 0x44;

/// Configure Accelerometer downsampling rates for FIFO
pub const FIFO_DOWNS: u8 = 0x45;

/// FIFO Watermark level LSB
pub const FIFO_WTM_0: u8 = 0x46;
/// FIFO Watermark level MSB
pub const FIFO_WTM_1: u8 = 0x47;

/// FIFO frame content configuration
pub const FIFO_CONFIG_0: u8 = 0x48;
/// FIFO frame content configuration
pub const FIFO_CONFIG_1: u8 = 0x49;

/// Auxillary interface slave device id
pub const AUX_DEV_ID: u8 = 0x4B;
/// Auxillary interface configuration
pub const AUX_IF_CONF: u8 = 0x4C;
/// Auxillary interface read register address
pub const AUX_RD_ADDR: u8 = 0x4D;
/// Auxillary interface write register address
pub const AUX_WR_ADDR: u8 = 0x4E;
/// Auxillary interface write data
pub const AUX_WR_DATA: u8 = 0x4F;

/// Configure the electrical behavior of the interrupt pins
pub const INT1_IO_CTRL: u8 = 0x53;
/// Configure the electrical behavior of the interrupt pins
pub const INT2_IO_CTRL: u8 = 0x54;
/// Configure interrupt modes
pub const INT_LATCH: u8 = 0x55;
/// Interrupt/Feature mapping on INT1
pub const INT1_MAP: u8 = 0x56;
/// Interrupt/Feature mapping on INT2
pub const INT2_MAP: u8 = 0x57;
/// Interrupt mapping hardware interrupts
pub const INT_MAP_DATA: u8 = 0x58;

/// Start initialization
pub const INIT_CTRL: u8 = 0x59;

/// Feature configuration
pub const RESERVED_REG_5B: u8 = 0x5B;
pub const RESERVED_REG_5C: u8 = 0x5C;

/// Feature configuration read/write port
pub const FEATURES_IN: u8 = 0x5E;

/// Internal error flags
pub const INTERNAL_ERROR: u8 = 0x5F;

/// NVM controller mode (Prog/Erase or Read only)
pub const NVM_CONF: u8 = 0x6A;

/// Serial interface settings
pub const IF_CONF: u8 = 0x6B;

/// Settings for the sensor self-test configuration and trigger
pub const ACC_SELF_TEST: u8 = 0x6D;

/// NVM backed configuration bits
pub const NV_CONF: u8 = 0x70;

/// Offset compensation for Accelerometer X-axis (NVM backed)
pub const OFFSET_0: u8 = 0x71;
/// Offset compensation for Accelerometer Y-axis (NVM backed)
pub const OFFSET_1: u8 = 0x72;
/// Offset compensation for Accelerometer Z-axis (NVM backed)
pub const OFFSET_2: u8 = 0x73;

/// Power mode configuration register
pub const PWR_CONF: u8 = 0x7C;

/// Sensor enable register
pub const PWR_CTRL: u8 = 0x7D;

/// Command register
pub const CMD: u8 = 0x7E;
