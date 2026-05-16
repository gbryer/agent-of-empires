[ -n "$AOE_INSTANCE_ID" ] || exit 0; mkdir -p "${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID" && printf %s "$1" > "${WORKDIR}/.aoe-hooks/$AOE_INSTANCE_ID/status"
