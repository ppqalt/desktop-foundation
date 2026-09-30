Services are event-driven state owners. The compositor adapter is the only current
resident service. Add notification/audio/clipboard providers only when a feature
needs them; never put polling or process launches in a surface binding.
