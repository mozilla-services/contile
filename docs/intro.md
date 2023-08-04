# Contile

Contile is a service that fetches tiles to support the Sponsored Tiles feature in Firefox. Sponsored Tiles are a set of display ads appearing on the Firefox New Tab page provided by either a paying partner, or a selected set of other partners.

## Table of Contents
- [api.md - API Documentation][1]
- [setup.md - Developer Setup Documentation][2]

[1]: ./api.md
[2]: ./setup.md


## Architecture

```mermaid
%%{init: {'theme':'dark'}}%%
flowchart 

subgraph GCS[GCS]
    ImageStore
    Filtering
end
subgraph Firefox
    NewTab
end

subgraph ContileDep[ ]
    Contile
    GCS
    MaxmindDb
end

Firefox <-->|v1/tiles| Contile
Firefox <--> ImageStore[(Tiles ImageStore)]
Filtering[(AMP Filtering)] --> Contile 
Shepherd -->|Settings File| Filtering
Contile --> MaxmindDb[(MaxmindDb)]
ImageStore --> Contile
Contile <-->|Tiles API| AMP["adMarketplace (AMP)"]
```