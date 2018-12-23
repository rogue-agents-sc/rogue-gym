"""test for ParallelRogueEnv"""
from rogue_gym.envs import ParallelRogueEnv
from data import CMD_STR, CMD_STR5, SEED1_DUNGEON, SEED1_DUNGEON2, SEED1_DUNGEON3


CONFIG_NOENEM = {
    "seed": 1,
}
NUM_WOKRERS = 8


def test_configs() -> None:
    env = ParallelRogueEnv(config_dicts=[CONFIG_NOENEM] * NUM_WOKRERS)
    for res in env.states:
        assert res.dungeon == SEED1_DUNGEON
    step = [CMD_STR, CMD_STR5]
    for i in range(len(CMD_STR)):
        action = ''.join(map(lambda x: step[x % 2][i], range(NUM_WOKRERS)))
        env.step(action)
    for i, res in enumerate(env.states):
        if i % 2 == 0:
            assert res.dungeon == SEED1_DUNGEON2
        else:
            assert res.dungeon == SEED1_DUNGEON3


def test_step_cyclic() -> None:
    env = ParallelRogueEnv(config_dicts=[CONFIG_NOENEM] * NUM_WOKRERS, max_steps=5)
    for i, c in enumerate(CMD_STR):
        action = ''.join([c] * NUM_WOKRERS)
        states, _, dones, _ = env.step(action)
        if i == 4:
            assert dones == [True] * NUM_WOKRERS
            for res in states:
                assert res.dungeon == SEED1_DUNGEON
        else:
            assert dones == [False] * NUM_WOKRERS
