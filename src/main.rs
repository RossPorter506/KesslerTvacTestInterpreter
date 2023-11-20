#![feature(iter_next_chunk)]

use anyhow::{Result, anyhow, bail};
use std::fs::File;
use std::io::{BufReader, BufRead};

const EMISSION_LINES: usize = 11;
const DEPLOYMENT_LINES: usize = 13;
const PAYLOAD_OFF_LINES: usize = 33;

const TVAC_EMISSION: Tvac = Tvac{packet_size: EMISSION_LINES, state: TvacState::Emission};
const TVAC_DEPLOYMENT: Tvac = Tvac{packet_size: DEPLOYMENT_LINES, state: TvacState::Deployment};
const TVAC_PAYLOAD_OFF: Tvac = Tvac{packet_size: PAYLOAD_OFF_LINES, state: TvacState::PayloadOff};

fn main() -> Result<()>{
	// Open text file
	let f = File::open("TVACTest.txt")?;
	let mut lines = BufReader::new(f).lines().flatten().peekable();

	let mut v: Vec<SensorData> = vec![];

	// Self-test at the beginning. Ignore for now.
	let r = lines.next_chunk::<220>();
	if r.is_err() { bail!("Failed to read initial self-test");}

	let mut state = TVAC_PAYLOAD_OFF;

	loop {
		// Get all the lines we need to parse one packet
		let chunk = match chunk_of(state.packet_size, &mut lines) {
			Ok(lns) => lns,
			Err(e) => {eprintln!("Failed to acquire chunk: {e}"); break},
		};

		// Interpret strings
		let packet = match state.interpret_packet(chunk) {
			Ok(data) => data,
			Err(e) => {eprintln!("Failed to parse chunk: {e}"); break},
		};

		// Save results to a vec for now
		v.push(packet);

		// Peek next line and see if it's a change of state. If so, update our state.
		let next_line = lines.peek();

		match next_line {
			Some(line) => {if let Some(new_state) = state_change(line) {state = new_state}},
			None => break, // No next line to get - end of file.
		}
	}
	// Do something with v
	println!("{v:?}");
	todo!()
}

/// Returns a vector of n elements, or an error if the iterator returns none.
fn chunk_of<E: std::fmt::Debug>(n:usize, iter: &mut dyn Iterator<Item=E>) -> Result<Vec<E>> {
	let mut vec = Vec::<E>::with_capacity(n);
	for _ in 0..n {
		vec.push(iter.next().ok_or(anyhow!("Could not take chunk, iterator ran out: {vec:?}"))?);
	}
	Ok(vec)
}

fn state_change(str: &str) -> Option<Tvac> {
	match str {
		"ENTERING EMISSION PHASE" => Some(TVAC_EMISSION),
		"ENTERING PINPULLER ACTIVATION PHASE" => Some(TVAC_DEPLOYMENT),
		"ENTERING PAYLOAD-OFF PHASE" => Some(TVAC_PAYLOAD_OFF),
		_ => None,
	}
}

fn interpret_payload_off_packet(arr: [String;11]) -> Result<SensorData> {
	Ok(
		SensorData::PayloadOff(
			PayloadOffSensors{
				time: Time {
					phase: extract_nth_word_as_number(&arr[0], 0)?,
					total: extract_nth_word_as_number(&arr[1], 0)?,
				},
				temp: Temperatures {
					lms_emit: extract_nth_word_as_number(&arr[2], 1)?,
					lms_rec: extract_nth_word_as_number(&arr[3], 1)?,
					msp: extract_nth_word_as_number(&arr[4], 1)?,
					heater: extract_nth_word_as_number(&arr[5], 1)?,
					hvdc: extract_nth_word_as_number(&arr[6], 1)?,
					tether_monitoring: extract_nth_word_as_number(&arr[7], 1)?,
					tether_connector: extract_nth_word_as_number(&arr[8], 1)?,
					msp_3v3_supply: extract_nth_word_as_number(&arr[9], 1)?,
				},
			}
		)
	)
}

fn interpret_deployment_packet(arr: [String;13]) -> Result<SensorData> {
	Ok(
		SensorData::Deployment(
			DeploymentSensors{
				time: Time {
					phase: extract_nth_word_as_number(&arr[0], 0)?,
					total: extract_nth_word_as_number(&arr[1],0)?,
				},
				pinpuller: Pinpuller {
					current: extract_measurement_from_nth_word(&arr[2], 3, "mA")?,
					acc: extract_measurement_from_nth_word(&arr[3], 3, "%")?
				},
				temp: Temperatures {
					lms_emit: extract_nth_word_as_number(&arr[4], 1)?,
					lms_rec: extract_nth_word_as_number(&arr[5], 1)?,
					msp: extract_nth_word_as_number(&arr[6], 1)?,
					heater: extract_nth_word_as_number(&arr[7], 1)?,
					hvdc: extract_nth_word_as_number(&arr[8], 1)?,
					tether_monitoring: extract_nth_word_as_number(&arr[9], 1)?,
					tether_connector: extract_nth_word_as_number(&arr[10], 1)?,
					msp_3v3_supply: extract_nth_word_as_number(&arr[11], 1)?,
				},
			}
		)
	)
}

fn interpret_emission_packet(arr: [String; 33]) -> Result<SensorData> {
	Ok(
		SensorData::Emission(
			EmissionSensors {
				time: Time {
					phase: extract_nth_word_as_number(&arr[0], 0)?,
					total: extract_nth_word_as_number(&arr[1], 0)?,
				},
				emitter: TetherSensors {
					cathode_offset: CathodeOffsetSupply {
						voltage: extract_measurement_from_nth_word(&arr[2], 3, "mV")?,
						current: extract_measurement_from_nth_word(&arr[3], 3, "uA")?,
						v_acc: extract_measurement_from_nth_word(&arr[7], 3, "%")?,
						c_acc: extract_measurement_from_nth_word(&arr[8], 3, "%")?,
					},
					tether_bias: TetherBiasSupply {
						voltage: extract_measurement_from_nth_word(&arr[9], 3, "mV")?,
						current: extract_measurement_from_nth_word(&arr[10], 3, "uA")?,
						v_acc: extract_measurement_from_nth_word(&arr[14],3, "%")?,
						c_acc: extract_measurement_from_nth_word(&arr[15], 3, "%")?,
					},
					heater: HeaterSupply {
						voltage: extract_measurement_from_nth_word(&arr[16], 3, "mV")?,
						current: extract_measurement_from_nth_word(&arr[17], 3, "mA")?,
						v_acc: extract_measurement_from_nth_word(&arr[20], 2, "%")?,
						c_acc: extract_measurement_from_nth_word(&arr[21], 2, "%")?
					},
					repeller: Repeller {
						voltage: extract_measurement_from_nth_word(&arr[22], 3, "mV")?,
						v_acc: extract_measurement_from_nth_word(&arr[24], 2, "%")?
					},
				},
				temp: Temperatures {
					lms_emit: extract_nth_word_as_number(&arr[2], 1)?,
					lms_rec: extract_nth_word_as_number(&arr[3], 1)?,
					msp: extract_nth_word_as_number(&arr[4], 1)?,
					heater: extract_nth_word_as_number(&arr[5], 1)?,
					hvdc: extract_nth_word_as_number(&arr[6], 1)?,
					tether_monitoring: extract_nth_word_as_number(&arr[7], 1)?,
					tether_connector: extract_nth_word_as_number(&arr[8], 1)?,
					msp_3v3_supply: extract_nth_word_as_number(&arr[9], 1)?,
				},
			}
		)
	)
}
/// Given a string like: `[ OK ] Measured output voltage: 259372mV` or `[FAIL] Measured output voltage: 259372mV`,
/// returns `259372` when provided appropriate word number and suffix to remove.
fn extract_measurement_from_nth_word<'a, T>(str: &'a str, n: usize, suffix: &'a str) -> Result<T>
	where T: std::str::FromStr,
	anyhow::Error: From<T::Err> {
	let cropped = remove_result_prefix(str);
	let measurement_and_unit = extract_nth_word(cropped, n)?;
	let measurement = measurement_and_unit.strip_suffix(suffix).ok_or(anyhow!("Failed to strip suffix '{suffix}' from '{measurement_and_unit}'"))?;
	measurement.parse().map_err(|_| {anyhow!("Failed to parse '{measurement}' as number")} )
}

fn remove_result_prefix(s: &str) -> &str {
	crop_first_n_letters(s, 7)
}

fn crop_first_n_letters(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((pos, _)) => &s[pos..],
        None => "",
    }
}

fn extract_nth_word(str: &str, n: usize) -> Result<&str> {
	str.split_ascii_whitespace().nth(n).ok_or(anyhow!("Failed to extract {n}th word from '{str}'"))
}

/// Given a string like `LMS Emitter: 78` returns 78 (as a number).
fn extract_nth_word_as_number<T>(str: &str, n: usize) -> Result<T>
where
	T: std::str::FromStr,
	anyhow::Error: From<T::Err>{
	let nth_word = str.split_ascii_whitespace().nth(n).ok_or(anyhow!("Failed to extract {n}th word from '{str}'"))?;
	nth_word.parse().map_err(|_| anyhow!("Failed to parse '{nth_word}' as number"))
}

#[derive(Debug)]
struct Tvac {
	packet_size: usize,
	state: TvacState,
}
impl Tvac {
	fn interpret_packet(&self, vec: Vec<String>) -> Result<SensorData>{
		match self.state {
			TvacState::PayloadOff => interpret_payload_off_packet(vec.try_into().map_err(|e| {eprintln!("{e:?}"); anyhow!("Failed to coerce to 11-sized arr")})?),
			TvacState::Deployment => interpret_deployment_packet(vec.try_into().map_err(|e| {eprintln!("{e:?}"); anyhow!("Failed to coerce to 13-sized arr")})?),
			TvacState::Emission => interpret_emission_packet(vec.try_into().map_err(|e| {eprintln!("{e:?}"); anyhow!("Failed to coerce to 33-sized arr")})?),
		}
	}
}

#[derive(Debug)]
enum TvacState {
	PayloadOff,
	Deployment,
	Emission,
}

#[derive(Debug)]
enum SensorData {
	PayloadOff(PayloadOffSensors),
	Deployment(DeploymentSensors),
	Emission(EmissionSensors),
}

#[derive(Debug)]
struct Time {
	total: u32,
	phase: u32,
}

#[derive(Debug)]
struct PayloadOffSensors{
	temp: Temperatures,
	time: Time,
}

#[derive(Debug)]
struct DeploymentSensors {
	temp: Temperatures,
	time: Time,
	pinpuller: Pinpuller,
}

#[derive(Debug)]
struct EmissionSensors {
	temp: Temperatures,
	time: Time,
	emitter: TetherSensors,
}

#[derive(Debug)]
struct Temperatures {
	lms_emit: u16,
	lms_rec: u16,
	msp: u16,
	heater: u16,
	hvdc: u16,
	tether_monitoring: u16,
	tether_connector: u16,
	msp_3v3_supply: u16,
}

#[derive(Debug)]
struct Pinpuller {
	current: u16,
	acc: f32,
}

#[derive(Debug)]
struct TetherSensors {
	cathode_offset: CathodeOffsetSupply,
	tether_bias: TetherBiasSupply,
	heater: HeaterSupply,
	repeller: Repeller,
}

#[derive(Debug)]
struct CathodeOffsetSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

#[derive(Debug)]
struct TetherBiasSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

#[derive(Debug)]
struct HeaterSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

#[derive(Debug)]
struct Repeller {
	voltage: i32,
	v_acc: f32,
}
