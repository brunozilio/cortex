import { ApiError } from "../api/http";

export const notAvailableError = () => new ApiError(501, "not_available", "Not available in this version");

export const notFoundError = () => new ApiError(404, "not_found", "Not found");

export const notAvailable = (): Promise<never> => Promise.reject(notAvailableError());

export const notFound = (): Promise<never> => Promise.reject(notFoundError());
