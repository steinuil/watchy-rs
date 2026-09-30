use core::convert::Infallible;

use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    dma, gpio,
    peripherals::{DMA_SPI3, GPIO10, GPIO18, GPIO19, GPIO23, GPIO5, GPIO9, SPI3},
    spi,
    time::Rate,
    Async,
};
use ssd1681_async::{
    BoosterConfig, BoosterPhase, BorderLut, BorderWaveform, BwPixel, DataEntryMode, DeepSleepMode,
    DisplayUpdateSequence, DriverOutput, GsTransitionControl, PatternSteps, RamWindow, RedPixel,
    Ssd1681,
};

type Bus<'a> = ExclusiveDevice<spi::master::SpiDma<'a, Async>, gpio::Output<'a>, Delay>;
type Controller<'a> = Ssd1681<Bus<'a>, gpio::Output<'a>, gpio::Output<'a>, gpio::Input<'a>, Delay>;

type BusError = embedded_hal_bus::spi::DeviceError<spi::Error, Infallible>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to initialize SPI: {0}")]
    SpiConfig(#[from] spi::master::ConfigError),

    #[error(transparent)]
    DmaBuf(#[from] esp_hal::dma::DmaBufError),

    #[error(transparent)]
    Controller(#[from] ssd1681_async::Error<BusError>),
}

pub struct Display<'a> {
    controller: Controller<'a>,
    border: BorderWaveform,
    state: PanelState,
}

#[derive(Debug)]
pub struct DisplayConfig<'a> {
    pub spi: SPI3<'a>,
    pub dma: DMA_SPI3<'a>,
    pub sclk: GPIO18<'a>,
    pub mosi: GPIO23<'a>,
    pub cs: GPIO5<'a>,
    pub dc: GPIO10<'a>,
    pub reset: GPIO9<'a>,
    pub busy: GPIO19<'a>,
}

pub type Frame = [u8; FRAME_LEN];
const FRAME_LEN: usize = Display::WIDTH as usize * Display::HEIGHT as usize / 8;

impl<'a> Display<'a> {
    pub const WIDTH: u16 = 200;
    pub const HEIGHT: u16 = 200;
    pub const FRAME_LEN: usize = FRAME_LEN;

    pub fn new(
        DisplayConfig {
            spi,
            dma,
            sclk,
            mosi,
            cs,
            dc,
            reset,
            busy,
        }: DisplayConfig<'a>,
    ) -> Result<Self, Error> {
        // Lowered from 20MHz because it got stuck on writing data.
        let spi_config = spi::master::Config::default()
            .with_frequency(Rate::from_mhz(16))
            .with_mode(spi::Mode::_0);

        let spi = spi::master::Spi::new(spi, spi_config)?
            .with_sck(sclk)
            .with_mosi(mosi)
            .with_dma(dma)
            .into_async();

        // 5120 bytes covers a 5000-byte plane in one transfer.
        let (rx_buffer, rx_desc, tx_buffer, tx_desc) = esp_hal::dma_buffers!(64, 5120);
        let rx = dma::DmaRxBuf::new(
            dma::aligned::DmaAlignedMut::new(rx_desc)?,
            dma::aligned::DmaAlignedMut::new(rx_buffer)?,
        )
        .expect("create rx DMA buffer");
        let mut tx = dma::DmaTxBuf::new(
            dma::aligned::DmaAlignedMut::new(tx_desc)?,
            dma::aligned::DmaAlignedMut::new(tx_buffer)?,
        )
        .expect("create tx DMA buffer");
        tx.set_burst_config(dma::BurstConfig::Enabled)
            .expect("enable tx burst");

        let out = gpio::OutputConfig::default();

        let bus = ExclusiveDevice::new(
            spi.with_buffers(rx, tx),
            gpio::Output::new(cs, gpio::Level::High, out),
            Delay,
        )
        .expect("exclusive SPI device");

        let timings = ssd1681_async::Timings {
            busy_timeout_us: 10_000_000,
            ..Default::default()
        };

        let controller = Ssd1681::new(
            bus,
            gpio::Output::new(dc, gpio::Level::High, out),
            gpio::Output::new(reset, gpio::Level::High, out),
            gpio::Input::new(busy, gpio::InputConfig::default().with_pull(gpio::Pull::Up)),
            Delay,
            timings,
        );

        Ok(Self {
            controller,
            border: Border::White.waveform(),
            state: PanelState::Hibernating,
        })
    }

    pub async fn clear(&mut self, color: BwPixel) -> Result<(), Error> {
        self.ensure_awake().await?;
        // Seems to interact in a weird manner with the panel
        self.controller
            .auto_write_bw_ram(color, PatternSteps::WHOLE_PANEL)
            .await?;
        self.controller
            .auto_write_red_ram(RedPixel::Red, PatternSteps::WHOLE_PANEL)
            .await?;
        Ok(())
    }

    pub async fn write_frame(&mut self, data: &Frame) -> Result<(), Error> {
        self.ensure_awake().await?;
        self.controller.write_bw_ram(FULL_FRAME, data).await?;
        Ok(())
    }

    pub async fn write_previous(&mut self, data: &Frame) -> Result<(), Error> {
        self.ensure_awake().await?;
        self.controller.write_red_ram(FULL_FRAME, data).await?;
        Ok(())
    }

    pub async fn refresh_full(&mut self) -> Result<(), Error> {
        self.ensure_initialized(false).await?;
        self.update(UPDATE_FULL).await
    }

    pub async fn refresh_partial(&mut self) -> Result<(), Error> {
        self.ensure_initialized(true).await?;
        self.update(UPDATE_PARTIAL).await
    }

    pub async fn draw(&mut self, frame: &Frame, full: bool) -> Result<(), Error> {
        self.write_frame(frame).await?;
        if full {
            self.refresh_full().await?;
        } else {
            self.refresh_partial().await?;
        }
        self.write_previous(frame).await?;
        Ok(())
    }

    pub async fn hibernate(&mut self) -> Result<(), Error> {
        self.controller.deep_sleep(DeepSleepMode::RetainRAM).await?;
        self.state = PanelState::Hibernating;
        Ok(())
    }

    pub async fn power_off(&mut self) -> Result<(), Error> {
        if !matches!(self.state, PanelState::Initialized { powered: true, .. }) {
            return Ok(());
        }

        self.update(POWER_OFF).await?;
        self.state = PanelState::Uninitialized;
        Ok(())
    }

    async fn init(&mut self, partial: bool) -> Result<(), Error> {
        if self.state == PanelState::Hibernating {
            self.controller.hardware_reset().await;
        }

        self.controller.software_reset().await?;
        self.controller.set_driver_output(DRIVER_OUTPUT).await?;
        self.controller.set_booster_soft_start(BOOSTER).await?;
        self.controller
            .select_temperature_sensor(ssd1681_async::TemperatureSensor::Internal)
            .await?;
        self.controller.set_border_waveform(self.border).await?;
        self.controller
            .set_data_entry_mode(DataEntryMode::default())
            .await?;
        self.controller.set_ram_window(FULL_FRAME).await?;

        self.state = PanelState::Initialized {
            partial,
            powered: false,
        };

        Ok(())
    }

    async fn power_on(&mut self) -> Result<(), Error> {
        match self.state {
            PanelState::Initialized { powered: true, .. } => Ok(()),
            PanelState::Initialized {
                powered: false,
                partial,
            } => {
                self.update(POWER_ON).await?;
                self.state = PanelState::Initialized {
                    partial,
                    powered: true,
                };
                Ok(())
            }
            _ => unreachable!("power_on should only be called after init"),
        }
    }

    async fn ensure_initialized(&mut self, partial: bool) -> Result<(), Error> {
        if matches!(self.state, PanelState::Initialized { partial: p, powered: true } if p == partial)
        {
            return Ok(());
        }

        self.init(partial).await?;
        self.power_on().await
    }

    async fn ensure_awake(&mut self) -> Result<(), Error> {
        if matches!(self.state, PanelState::Initialized { powered: true, .. }) {
            return Ok(());
        }
        self.init(false).await?;
        self.power_on().await
    }

    async fn update(&mut self, sequence: DisplayUpdateSequence) -> Result<(), Error> {
        self.controller
            .set_display_update_sequence(sequence)
            .await?;
        self.controller.master_activation().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelState {
    Hibernating,
    Uninitialized,
    Initialized { partial: bool, powered: bool },
}

const DRIVER_OUTPUT: DriverOutput = match DriverOutput::new(Display::HEIGHT) {
    Ok(v) => v,
    Err(_) => unreachable!(),
};

const BOOSTER: BoosterConfig = {
    let phase = BoosterPhase {
        driving_strength: ssd1681_async::PhaseDrivingStrength::_8,
        min_off_time: ssd1681_async::PhaseMinOffTime::_2_6,
        duration: ssd1681_async::PhaseDuration::_10ms,
    };

    BoosterConfig {
        phase1: phase,
        phase2: phase,
        phase3: phase,
    }
};

const FULL_FRAME: RamWindow =
    match RamWindow::new(0, (Display::WIDTH / 8 - 1) as u8, 0, Display::HEIGHT - 1) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    };

const POWER_ON: DisplayUpdateSequence = DisplayUpdateSequence::ENABLE_CLOCK_SIGNAL
    .union(DisplayUpdateSequence::ENABLE_ANALOG)
    .union(DisplayUpdateSequence::LOAD_TEMPERATURE_VALUE)
    .union(DisplayUpdateSequence::LOAD_LUT)
    .union(DisplayUpdateSequence::USE_DISPLAY_MODE_2);

const POWER_OFF: DisplayUpdateSequence = DisplayUpdateSequence::ENABLE_CLOCK_SIGNAL
    .union(DisplayUpdateSequence::DISABLE_ANALOG)
    .union(DisplayUpdateSequence::DISABLE_CLOCK_SIGNAL);

const UPDATE_FULL: DisplayUpdateSequence = DisplayUpdateSequence::ENABLE_CLOCK_SIGNAL
    .union(DisplayUpdateSequence::ENABLE_ANALOG)
    .union(DisplayUpdateSequence::LOAD_TEMPERATURE_VALUE)
    .union(DisplayUpdateSequence::LOAD_LUT)
    .union(DisplayUpdateSequence::DISPLAY);

const UPDATE_PARTIAL: DisplayUpdateSequence =
    UPDATE_FULL.union(DisplayUpdateSequence::USE_DISPLAY_MODE_2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Border {
    #[default]
    White,
    Black,
}

impl Border {
    const fn waveform(self) -> BorderWaveform {
        match self {
            Border::White => BorderWaveform::GsTransition {
                lut: BorderLut::LUT1,
                control: GsTransitionControl::FollowLUT,
            },
            Border::Black => BorderWaveform::GsTransition {
                lut: BorderLut::LUT2,
                control: GsTransitionControl::VCOMAtRed,
            },
        }
    }
}
