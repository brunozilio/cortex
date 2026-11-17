/** @type {import("../types").ToolSpec} */
export const spec = {
  name: "AskUserQuestion",
  input: {
    "properties": {
      "questions": {
        "items": {
          "properties": {
            "header": {
              "description": "A tag of at most 12 characters.",
              "type": "string"
            },
            "multiSelect": {
              "default": false,
              "type": "boolean"
            },
            "options": {
              "items": {
                "properties": {
                  "description": {
                    "type": "string"
                  },
                  "label": {
                    "type": "string"
                  }
                },
                "required": [
                  "label",
                  "description"
                ],
                "type": "object"
              },
              "maxItems": 4,
              "minItems": 2,
              "type": "array"
            },
            "question": {
              "description": "The whole question, ending with a question mark.",
              "type": "string"
            }
          },
          "required": [
            "question",
            "header",
            "options",
            "multiSelect"
          ],
          "type": "object"
        },
        "maxItems": 4,
        "minItems": 1,
        "type": "array"
      }
    },
    "required": [
      "questions"
    ],
    "type": "object"
  },
  description: `Ask the user one to four questions, each with two to four options, when you cannot go on without a decision that is genuinely theirs. The user can also answer in their own words. If you recommend an option, put it first and end its label with "(Recommended)". Ask only when the answer changes what you do next: not for a choice with a conventional default nor for a fact you can check in the code yourself; then pick the obvious option, say so and go on. The user may see only the question, so make each one stand on its own. In planning mode, ask to clarify requirements or choose between approaches, never whether the plan is ready or whether to proceed.`,
};

/**
 * @param {import("../types").ToolInput} input
 * @param {import("../types").ToolContext} ctx
 */
export function execute(input, ctx) {
  return ctx.builtin(input);
}
