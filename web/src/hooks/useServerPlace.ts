import { instanceQuery } from "@agenteschat/client";
import { type ServerPlace, serverPlace } from "@agenteschat/client/lib/server";
import { useQuery } from "@tanstack/react-query";
import { useApi } from "@/api/context";

export function useServerPlace(): ServerPlace | null {
  const api = useApi();
  const instance = useQuery(instanceQuery(api));
  const cloud = instance.data?.cloud;
  if (cloud === undefined) return null;
  return serverPlace(cloud, false);
}
