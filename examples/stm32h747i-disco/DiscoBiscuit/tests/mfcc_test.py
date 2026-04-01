import numpy as np
from scipy.signal import sawtooth, square
from python_speech_features import mfcc

def mfcc_for_wave(wave, samplerate=16000):
    return mfcc(wave, samplerate=samplerate)

def generate_signals(samplerate):
    t = np.linspace(0, 1, samplerate, endpoint=False)
    return {
        "sine": 0.5 * np.sin(2 * np.pi * 440 * t),
        "noise": np.random.randn(samplerate),
        "square": 0.5 * square(2 * np.pi * 440 * t),
        "sawtooth": 0.5 * sawtooth(2 * np.pi * 440 * t),
    }


def test_mfcc_nonzero():
    samplerate = 16000
    np.random.seed(0)
    signals = generate_signals(samplerate)
    for name, wave in signals.items():
        features = mfcc_for_wave(wave, samplerate)
        assert np.any(
            features != 0.0
        ), f"{name} MFCC should not be all zeros"


if __name__ == "__main__":
    test_mfcc_nonzero()
    print("MFCC test passed")
