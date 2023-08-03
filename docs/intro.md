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
flowchart TD
    Firefox <-->|v1/tiles| Contile
    Firefox <--> ImageStore[(GCS <br/> Tiles ImageStore)]
    Contile --> ImageStore
    Filtering[(GCS <br/> AMP Filtering)] --> Contile
    Contile <-->|Tiles API| AMP["adMarketplace (AMP)" ]
    Shepherd -->|AMP settings json| Filtering
    MaxmindDb[(MaxmindDb)] --> Contile
subgraph ContileDependencies[ ]
    Contile
    ImageStore
    MaxmindDb
    Filtering

end

subgraph Firefox
   NewTab
end


```