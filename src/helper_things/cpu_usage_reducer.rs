use std::time::Instant;
use rayon::iter::ParallelIterator;
use rayon::prelude::ParallelSlice;

// Chosen from OS textbook, looking at graphs for scheduling functions
const ALPHA: f32 = 0.6;
const COEFFICIENT_FACTOR: f32 = 1.0 - ALPHA;

// Five seems to be enough for history, given the exponential nature of the function- any index past
// 5 would not really effect how much time is given to a single thread
const PREVIOUS_FRAMES: usize = 5;

const FRAME_COEFFICIENTS: [f32; PREVIOUS_FRAMES] =
    [
        ALPHA,
        ALPHA * COEFFICIENT_FACTOR,
        ALPHA * COEFFICIENT_FACTOR * COEFFICIENT_FACTOR,
        ALPHA * COEFFICIENT_FACTOR * COEFFICIENT_FACTOR * COEFFICIENT_FACTOR,
        ALPHA * COEFFICIENT_FACTOR * COEFFICIENT_FACTOR * COEFFICIENT_FACTOR * COEFFICIENT_FACTOR
    ];

// Used to ensure that a single thread is always given a little bit of time to run, even if past
// calls to the function that is executed happened to finish very quickly
const MAX_ONE_THREAD_COEFFICIENT: f32 = 0.10;

type TimeTaken = f32;

/// Determines how much time to allocate to computing with a single thread instead of several based
/// upon how long previous computations with a single thread took
pub struct TimeTakeHistory
{
    one_thread: [TimeTaken; PREVIOUS_FRAMES],
    total_time_taken_micro_seconds: f32,
    time_passed: Instant,
}

impl TimeTakeHistory
{
    /// Creates a new time history structure that assumes previous iterations of the single thread
    /// use the maximum allocated time for a single thread
    pub fn new() -> TimeTakeHistory
    {
        let assumed_previous_frame_time = 16_000.0; // microseconds

        TimeTakeHistory
        {
            one_thread: [assumed_previous_frame_time * MAX_ONE_THREAD_COEFFICIENT; PREVIOUS_FRAMES],
            total_time_taken_micro_seconds: assumed_previous_frame_time,
            time_passed: Instant::now()
        }
    }

    /// Executes the given function with a single thread. If the amount of time allocated for a single
    /// thread exceeds its time limit, the rest of the data is executed with the function using several threads
    ///
    /// `time_taken` - history of time taken by a single thread for previous calls of the provided function
    /// `f` - the function to execute on the given data
    /// `data` - the data which the given function operates on
    pub fn apply_to_function<T: Sync, F: Fn(&[T]) + Sync>(time_taken: &mut TimeTakeHistory, f: F, data: &Vec<T>)
    {
        time_taken.start_frame();
        let mut number_elements_processed = 0;

        // *** One thread ***

        let time_taken_one_thread = Instant::now();
        let max_time_one_thread = time_taken.time_allowed_one_thread();

        while (time_taken_one_thread.elapsed().as_micros() as f32) < max_time_one_thread
        {
            if number_elements_processed >= data.len()
            {
                return;
            }

            let _ = data[number_elements_processed..number_elements_processed + 1].chunks(1).map(|x| f(x)).collect::<()>();
            number_elements_processed += 1;
        }

        // Rotating to the right and writing to the first index effectively overwrites the last index
        time_taken.one_thread.rotate_right(1);
        // Record how long the most recent call of the function using a single thread took. It will
        // have the most influence of how much time to allocate to a single thread the next time
        // the function is called. The index written to must match the index of the largest frame coefficient
        time_taken.one_thread[0] = time_taken_one_thread.elapsed().as_micros() as f32;


        // Gimme all of those threads

        data[number_elements_processed..].par_chunks(1).map(|x| f(x)).collect::<()>();

        time_taken.end_frame();
    }

    /// Calculates how much time to allocate to a single thread when executing a function
    pub fn time_allowed_one_thread(&mut self) -> f32
    {
        // A minimum amount of time is given to the single thread, to reduce possibility of quirks
        // in the amount of data a function had to execute caused the function to finish quickly,
        // which would not give any time for single thread in future calls

        self.one_thread
            .iter()
            .zip(FRAME_COEFFICIENTS.iter())
            .map(|(time_taken, coefficient)| time_taken * coefficient)
            .sum::<f32>().min(self.total_time_taken_micro_seconds * MAX_ONE_THREAD_COEFFICIENT)
    }

    /// Resets the counter for how much time has passed
    pub fn start_frame(&mut self)
    {
        self.time_passed = Instant::now();
    }

    /// Calculates how much time in total has passed executing the last call of the function provided
    /// to this instance of the TimeTakenHistory structure (single thread + several threads)
    pub fn end_frame(&mut self)
    {
        let time_passed = self.time_passed.elapsed().as_micros();

        if time_passed == 0 // Platform does not support measuring micro seconds (unlikely all operations took 0 micro seconds)
        {
            self.total_time_taken_micro_seconds = time_passed as f32;
        }
        else
        {
            self.total_time_taken_micro_seconds = 1000.0; // Default to 1ms, smallest amount of time every platform
            // should be able to measure
        }
    }
}