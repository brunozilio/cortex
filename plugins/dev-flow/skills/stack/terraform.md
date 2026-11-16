# Terraform on AWS

## What you may run

| Command | When |
|---|---|
| `terraform fmt -recursive` | always, before committing |
| `terraform init -backend=false` then `terraform validate` | always |
| `terraform plan -out tf.plan` | when the person's AWS session allows it; read-only |
| `terraform apply`, `destroy`, `import`, `state ...` | never on your own |

Applying is the pipeline's job after the merge, or the person's. The
flow's gate asks before any `apply` or `destroy`; even when asked to run
one, show the plan first and name the workspace and account
(`aws sts get-caller-identity`). Never edit state by hand and never
commit `tf.plan`, `.terraform/` or a `*.tfvars` that holds a secret.

## Code

- Follow the repository's layout (modules, one folder per environment or
  workspaces) and its naming and tagging: every resource carries the
  tags the neighbours carry.
- A value that differs per environment is a variable with a description
  and a type; nothing environment-specific is hard-coded in a module.
- Pin provider and module versions the way the repository does; do not
  bump them inside a story.
- IAM is least privilege: name the actions and the resource ARNs. A
  `"*"` in `Action` or `Resource` needs a reason in the PR.
- Nothing is public unless the spec says so: no open security group
  (`0.0.0.0/0`) beyond what is already exposed, buckets private and
  encrypted, data stores encrypted at rest.
- Secrets live in Secrets Manager or Parameter Store and are referenced,
  never written as a default or a literal.

## Reviewing a plan

Read the whole plan, not its summary line. In the PR, list what is
created, changed and destroyed. Stop and ask when the plan:

- destroys or replaces (`-/+`) a stateful resource: a database, a
  bucket, a queue with messages, a KMS key, a DNS record in use;
- changes something the story did not mention (drift: say so, do not
  fold it in);
- touches IAM or network exposure more widely than the spec describes.

A change that must not interrupt service is done in steps (create the
new, move, remove the old) across more than one PR, and the spec's
rollout section says which.
