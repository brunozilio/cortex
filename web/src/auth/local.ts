import type { AuthClient, AuthResult } from "./client";

export const LOCAL_AUTH_REFUSAL: AuthResult = {
  ok: false,
  status: 501,
  code: "not_available",
  message: "not_available",
};

const refused = async (): Promise<AuthResult> => LOCAL_AUTH_REFUSAL;

export function createLocalAuthClient(): AuthClient {
  return {
    signIn: refused,
    signUp: refused,
    updateName: refused,
    sendVerificationEmail: refused,
    signInSocial: refused,
    requestPasswordReset: refused,
    resetPassword: refused,
    signOut: async () => {},
    signInOneTime: refused,
    ownerClaim: refused,
  };
}
