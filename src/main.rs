#![feature(iter_next_chunk)]

use anyhow::{Result, anyhow};
use std::fs::File;
use std::io::{BufReader, BufRead};

fn main() -> Result<()>{
	// Open text file
	let f = File::open("TVACTest.txt")?;
	let mut lines = BufReader::new(f).lines().flatten().peekable();

	let mut v: Vec<SensorData> = vec![];
	
	let mut state = TvacTestState::SelfTest;

	let mut ok = true;
	while ok {
		match state {
			TvacTestState::SelfTest => {
				// Try get the next 220 lines
				let r = lines.next_chunk::<220>();
				if r.is_err() {
					ok = false;
				}
			}
			TvacTestState::PayloadOff => {
				// Try get the next 11 lines.
				// Interpret chunk.
				// Store results
				let r = lines.next_chunk::<11>();
				if let Ok(lines) = r {
					let packet = interpret_payload_off_packet(lines)?;
					v.push(SensorData::PayloadOff(packet))
				}
				else {
					ok = false;
				}
			}
			TvacTestState::Deployment => {
				// Try get the next 13 lines.
				// Interpret chunk.
				// Store results
				let r = lines.next_chunk::<13>();
				if let Ok(lines) = r {
					let packet = interpret_deployment_packet(lines)?;
					v.push(SensorData::Deployment(packet))
				}
				else {
					ok = false;
				}
			}
			TvacTestState::Emission => {
				// try get the next 33 lines.
				// Interpret chunk.
				// Store results
				let r = lines.next_chunk::<33>();
				if let Ok(lines) = r {
					let packet = interpret_emission_packet(lines)?;
					v.push(SensorData::Emission(packet))
				}
				else {
					ok = false;
				}
			}
		}
		// Peek next line and see if it's a change of state. If so, update our state.
		let next_line = lines.peek();

		match next_line {
			Some(line) => {if let Some(new_state) = state_change(line) {state = new_state}},
			None => {ok = false;} // No next line to get - end of file.
		}
	}
	// Do something with v
	todo!()
}

fn state_change(str: &str) -> Option<TvacTestState> {
	match str {
		"ENTERING EMISSION PHASE" => Some(TvacTestState::Emission),
		"ENTERING PINPULLER ACTIVATION PHASE" => Some(TvacTestState::Deployment),
		"ENTERING PAYLOAD-OFF PHASE" => Some(TvacTestState::PayloadOff),
		_ => None,
	}
}

fn interpret_payload_off_packet(arr: [String; 11]) -> Result<PayloadOffSensors> {
	Ok(
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
}

fn interpret_deployment_packet(arr: [String; 13]) -> Result<DeploymentSensors> {
	Ok(
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
}

fn interpret_emission_packet(arr: [String; 33]) -> Result<EmissionSensors> {
	Ok(
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
}
/// Given a string like: `[ OK ] Measured output voltage: 259372mV` or `[FAIL] Measured output voltage: 259372mV`,
/// returns `259372` when provided appropriate word number and suffix to remove.
fn extract_measurement_from_nth_word<'a, T>(str: &'a str, n: usize, suffix: &'a str) -> Result<T>
	where T: std::str::FromStr,
	anyhow::Error: From<T::Err> {
	let cropped = remove_result_prefix(str);
	let measurement_and_unit = extract_nth_word(cropped, n)?;
	let measurement = measurement_and_unit.strip_suffix(suffix).ok_or(anyhow!("Failed to strip suffix"))?;
	measurement.parse().map_err(|_| anyhow!("Failed to parse number") )
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
	str.split_ascii_whitespace().nth(n).ok_or(anyhow!("Failed to extract"))
}

/// Given a string like `LMS Emitter: 78` returns 78 (as a number).
fn extract_nth_word_as_number<T>(str: &str, n: usize) -> Result<T> 
where
	T: std::str::FromStr,
	anyhow::Error: From<T::Err>{
	let nth_word = str.split_ascii_whitespace().nth(n).ok_or(anyhow!("Failed to extract"))?;
	nth_word.parse().map_err(|_| anyhow!("Failed to parse number"))
}

trait SubState {
	fn next(&mut self) -> Option<Self> where Self: std::marker::Sized;
}

enum TvacTestState {
	SelfTest,
	PayloadOff,
	Deployment,
	Emission
}

enum SensorData {
	PayloadOff(PayloadOffSensors),
	Deployment(DeploymentSensors),
	Emission(EmissionSensors),
}

struct Time {
	total: u32,
	phase: u32,
}

struct PayloadOffSensors{
	temp: Temperatures,
	time: Time,
}

struct DeploymentSensors {
	temp: Temperatures,
	time: Time,
	pinpuller: Pinpuller,
}

struct EmissionSensors {
	temp: Temperatures,
	time: Time,
	emitter: TetherSensors,
}

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

struct Pinpuller {
	current: u16,
	acc: f32,
}

struct TetherSensors {
	cathode_offset: CathodeOffsetSupply,
	tether_bias: TetherBiasSupply,
	heater: HeaterSupply,
	repeller: Repeller,
}

struct CathodeOffsetSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

struct TetherBiasSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

struct HeaterSupply {
	voltage: i32,
	current: i32,
	v_acc: f32,
	c_acc: f32,
}

struct Repeller {
	voltage: i32,
	v_acc: f32,
}
