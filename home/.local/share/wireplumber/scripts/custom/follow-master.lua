SimpleEventHook {
  name = "custom/follow-master-output",
  interests = {
    EventInterest {
      Constraint { "event.type", "=", "metadata-changed" },
      Constraint { "metadata.name", "=", "default" },
      Constraint { "event.subject.key", "=", "default.configured.audio.sink" },
    },
  },
  execute = function (event)
    local source = event:get_source ()
    local metadata = source:call ("get-object-manager", "metadata"):lookup {
      Constraint { "metadata.name", "=", "default" }
    }
    if not metadata then
      return
    end
    local nodes_om = source:call ("get-object-manager", "node")
    for node in nodes_om:iterate {
      Constraint { "media.class", "=", "Stream/Output/Audio" }
    } do
      local id = node ["bound-id"]
      metadata:set (id, "target.object", nil, nil)
      metadata:set (id, "target.node", nil, nil)
    end
  end
}:register ()
