#![feature(iter_next_chunk)]

use anyhow::{Result, anyhow, bail};
use serde::ser::SerializeStruct;
use std::fs::File;
use std::io::{BufReader, BufRead};
use csv::WriterBuilder;

const SELF_TEST_LINES: usize= 221;
const EMISSION_LINES: usize = 33;
const DEPLOYMENT_LINES: usize = 13;
const PAYLOAD_OFF_LINES: usize = 11;

const TVAC_EMISSION: Tvac = Tvac{packet_size: EMISSION_LINES, state: PayloadState::Emission};
const TVAC_DEPLOYMENT: Tvac = Tvac{packet_size: DEPLOYMENT_LINES, state: PayloadState::Deployment};
const TVAC_PAYLOAD_OFF: Tvac = Tvac{packet_size: PAYLOAD_OFF_LINES, state: PayloadState::PayloadOff};

fn main() -> Result<()>{
	// Open text file
	let f = File::open("TVACTest-original.txt")?;
	let mut lines = BufReader::new(f).lines().flatten().peekable();

	let mut v: Vec<SensorData> = vec![];

	// Self-test at the beginning. Ignore for now.
	let r = lines.next_chunk::<SELF_TEST_LINES>();
	if r.is_err() { bail!("Failed to read initial self-test");}

	let mut state = TVAC_PAYLOAD_OFF;

	while let Some(next_line) = lines.peek() {
		if next_line.is_empty() {
			lines.next();
			continue;
		}

		// See if the next line announces a change of state. If so, update our state.
		if let Some(new_state) = state_change(next_line) {
			state = new_state;
			lines.next();
		}

		// Get all the lines we need to parse one packet
		let chunk = match chunk_of(state.packet_size, &mut lines) {
			Ok(lns) => lns,
			Err(e) => {eprintln!("{e}"); break},
		};

		// Interpret chunk
		let packet = match state.interpret_packet(chunk.clone()) {
			Ok(data) => data,
			Err(e) => {
				eprintln!("Failed to parse chunk: {e}. Chunk written to failed_chunks."); 
				write_broken_chunk_to_file(chunk); 
				let mut line = lines.next();
				while line.is_some() && !(line.unwrap()).is_empty() {
					line = lines.next();
				}
				continue},
		};

		// Save results to a vec for now
		v.push(packet);
	}
	// Do something with v
	//println!("{v:?}");
	let mut wtr = WriterBuilder::new().has_headers(false).flexible(true).from_path("out.csv")?;

	wtr.write_record([
		"Payload state", 
		"Total time (s)", 
		"Phase time (s)", 
		"LMS emitter temp (°C)",
		"LMS receiver temp (°C)",
		"MSP temp (°C)",
		"Heater temp (°C)",
		"HVDC supply temp (°C)",
		"Tether monitoring temp (°C)",
		"Tether connector temp (°C)",
		"MSP 3V3 supply temp (°C)",
		"Pinpuller current (mA)",
		"Pinpuller accuracy (%)",
		"Cathode offset voltage (mV)",
		"Cathode offset current (uA)",
		"Cathode offset voltage accuracy (%)",
		"Cathode offset current accuracy (%)",
		"Tether bias voltage (mV)",
		"Tether bias current (uA)",
		"Tether bias voltage accuracy (%)",
		"Tether bias current accuracy (%)",
		"Heater voltage (mV)",
		"Heater current (mA)",
		"Heater voltage accuracy (%)",
		"Heater current accuracy (%)",
		"Repeller voltage (mV)",
		"Repeller voltage accuracy (%)",])?;
	for record in v {
		wtr.serialize(record)?;
	}

	println!("Written to out.csv");
	Ok(())
}



fn write_broken_chunk_to_file(chunk: Vec<String>) {
	static mut FILE_NUM: u32 = 0;
	let file_path = unsafe{ format!("failed_chunks/chunk{FILE_NUM}.txt") };
	unsafe{FILE_NUM += 1};
	std::fs::write(file_path, chunk.join("\n")).unwrap();
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
		SensorData {
			time: extract_time(&arr[0..2])?,
			temperatures: extract_temperatures(&arr[2..10])?,
			state: PayloadState::PayloadOff,
			tether: None,
			pinpuller: None,
		}
	)
}

fn interpret_deployment_packet(arr: [String;13]) -> Result<SensorData> {
	Ok(
		SensorData{
			time: extract_time(&arr[0..2])?,
			temperatures: extract_temperatures(&arr[4..12])?,
			state: PayloadState::Deployment,
			pinpuller: Some( Pinpuller {
				current: extract_measurement_from_nth_word(&arr[2], 3, "mA")?,
				acc: extract_measurement_from_nth_word(&arr[3], 3, "%")?
			}),
			tether: None,
		}
	)
}

fn interpret_emission_packet(arr: [String; 33]) -> Result<SensorData> {
	Ok(
		SensorData {
			time: extract_time(&arr[0..2])?,
			temperatures: extract_temperatures(&arr[25..33])?,
			state: PayloadState::Emission,
			tether: Some(TetherSensors {
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
			}),
			pinpuller: None,
		}
	)
}

fn extract_temperatures(slice: &[String]) -> Result<Temperatures> {
	Ok(Temperatures {
		lms_emit: extract_nth_word_as_number(&slice[0], 2)?,
		lms_rec: extract_nth_word_as_number(&slice[1], 2)?,
		msp: extract_nth_word_as_number(&slice[2], 1)?,
		heater: extract_nth_word_as_number(&slice[3], 2)?,
		hvdc: extract_nth_word_as_number(&slice[4], 2)?,
		tether_monitoring: extract_nth_word_as_number(&slice[5], 2)?,
		tether_connector: extract_nth_word_as_number(&slice[6], 2)?,
		msp_3v3_supply: extract_nth_word_as_number(&slice[7], 3)?,
	})
}

fn extract_time(slice: &[String]) -> Result<Time> {
	Ok(Time {
		phase_time: extract_nth_word_as_number(&slice[0], 0)?,
		total_time: extract_nth_word_as_number(&slice[1], 0)?,
	})
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
	str.split_ascii_whitespace().nth(n).ok_or(anyhow!("Failed to extract {n}th word from '{str}': `{str}`"))
}

/// Given a string like `LMS Emitter: 78` returns 78 (as a number).
fn extract_nth_word_as_number<T>(str: &str, n: usize) -> Result<T>
where
	T: std::str::FromStr,
	anyhow::Error: From<T::Err>{
	
	let nth_word = str.split_ascii_whitespace().nth(n).ok_or(anyhow!("Failed to extract {n}th word from '{str}'"))?;
	nth_word.parse().map_err(|_| anyhow!("Failed to parse '{nth_word}' as number: `{str}`"))
}

#[derive(Debug)]
struct Tvac {
	packet_size: usize,
	state: PayloadState,
}
impl Tvac {
	fn interpret_packet(&self, vec: Vec<String>) -> Result<SensorData>{
		match self.state {
			PayloadState::PayloadOff => interpret_payload_off_packet(vec.try_into().map_err(|e| {eprintln!("{e:?}"); anyhow!("Failed to coerce to 11-sized arr")})?),
			PayloadState::Deployment => interpret_deployment_packet(vec.try_into().map_err(|e| {eprintln!("{e:?}"); anyhow!("Failed to coerce to 13-sized arr")})?),
			PayloadState::Emission => interpret_emission_packet(vec.try_into().map_err(|e| {eprintln!("{e:?}"); anyhow!("Failed to coerce to 33-sized arr")})?),
		}
	}
}

#[derive(Debug, serde::Serialize)]
enum PayloadState {
	PayloadOff,
	Deployment,
	Emission,
}

#[derive(Debug)]
struct SensorData {
	time: Time,
	temperatures: Temperatures,
	state: PayloadState,
	pinpuller: Option<Pinpuller>,
	tether: Option<TetherSensors>,
}
impl serde::Serialize for SensorData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SensorData", 3)?;
		state.serialize_field("state", &self.state)?;
        state.serialize_field("time", &self.time)?;
		state.serialize_field("temperatures", &self.temperatures)?;
		match &self.pinpuller {
			Some(p) => state.serialize_field("pinpuller", &p),
			None => {
				state.serialize_field("current", &None::<Pinpuller>)?;
				state.serialize_field("acc", &None::<Pinpuller>)?;
				Ok(())
			}
		}?;
		match &self.tether {
			Some(t) => state.serialize_field("tether", &t),
			None => {
				state.serialize_field("cathode_offset", &None::<CathodeOffsetSupply>)?;
				state.serialize_field("tether_bias", &None::<TetherBiasSupply>)?;
				state.serialize_field("heater", &None::<HeaterSupply>)?;
				state.serialize_field("repeller", &None::<Repeller>)?;
				Ok(())
			}
		}?;
        state.end()
    }
}

#[derive(Debug, serde::Serialize)]
struct Time {
	total_time: u32,
	phase_time: u32,
}

#[derive(Debug, serde::Serialize)]
struct PayloadOffSensors{
	temp: Temperatures,
	time: Time,
}

#[derive(Debug, serde::Serialize)]
struct DeploymentSensors {
	temp: Temperatures,
	time: Time,
	pinpuller: Pinpuller,
}

#[derive(Debug, serde::Serialize)]
struct EmissionSensors {
	temp: Temperatures,
	time: Time,
	emitter: TetherSensors,
}

#[derive(Debug, serde::Serialize)]
struct Temperatures {
	lms_emit: i16,
	lms_rec: i16,
	msp: i16,
	heater: i16,
	hvdc: i16,
	tether_monitoring: i16,
	tether_connector: i16,
	msp_3v3_supply: i16,
}

#[derive(Debug, serde::Serialize)]
struct Pinpuller {
	current: u16,
	acc: f32,
}

#[derive(Debug, serde::Serialize)]
struct TetherSensors {
	cathode_offset: CathodeOffsetSupply,
	tether_bias: TetherBiasSupply,
	heater: HeaterSupply,
	repeller: Repeller,
}

#[derive(Debug, serde::Serialize)]
struct CathodeOffsetSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

#[derive(Debug, serde::Serialize)]
struct TetherBiasSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

#[derive(Debug, serde::Serialize)]
struct HeaterSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

#[derive(Debug, serde::Serialize)]
struct Repeller {
	voltage: i32,
	v_acc: f32,
}
