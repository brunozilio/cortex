/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "ObserverReport",
  input: {
    "additionalProperties": false,
    "properties": {
      "message": {
        "description": "What the agent you watch should know, in a few plain sentences: what you saw and why it matters.",
        "type": "string"
      }
    },
    "required": [
      "message"
    ],
    "type": "object"
  },
  description: `Sends one note to the agent you watch. It reaches that agent between its steps as advice from its observer, never as the user's words. Use it only for something worth interrupting for: a mistake about to compound, a constraint the agent missed, prior art it does not know about. Most of the time, say nothing.`,
};
