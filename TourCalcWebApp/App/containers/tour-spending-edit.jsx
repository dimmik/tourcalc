import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Autocomplete from './Autocomplete.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

import InputLabel from '@material-ui/core/InputLabel';
import Input from '@material-ui/core/Input';
import MenuItem from '@material-ui/core/MenuItem';
import FormHelperText from '@material-ui/core/FormHelperText';
import FormControl from '@material-ui/core/FormControl';
import Select from '@material-ui/core/Select';
import TextField from '@material-ui/core/TextField';
import Chip from '@material-ui/core/Chip';
import Switch from '@material-ui/core/Switch';
import Checkbox from '@material-ui/core/Checkbox';
import ListItemText from '@material-ui/core/ListItemText';
import FormControlLabel from '@material-ui/core/FormControlLabel';
import FormGroup from '@material-ui/core/FormGroup';
import Button from '@material-ui/core/Button';

const ITEM_HEIGHT = 48;
const ITEM_PADDING_TOP = 8;
const MenuProps = {
    PaperProps: {
        style: {
            maxHeight: ITEM_HEIGHT * 4.5 + ITEM_PADDING_TOP,
            width: 250,
        },
    },
};


export default class SpendingsForm extends React.Component {
    constructor(props) {
        super(props);
        this.tour = props.tour
        if (props.spending != null) this.spending = JSON.parse(JSON.stringify(props.spending))
        else {
            this.spending.fromGuid = this.tour.persons.length > 0 ? this.tour.persons[0].guid : ""
            this.spending.type = "Common"
        }
        this.state = {
            dialogOpen: props.open,
            spending:  this.spending
        }


    }

    componentWillReceiveProps(props) {
        this.tour = props.tour
        if (props.spending != null) { // edit
            this.spending = JSON.parse(JSON.stringify(props.spending))
        }
        else { // adding
            this.spending.description = ""
            this.spending.amountInCents = 0;
            this.spending.type = "Common";
            this.spending.fromGuid = this.spending.fromGuid == null ? (this.tour.persons.length > 0 ? this.tour.persons[0].guid : "") : this.spending.fromGuid
        }
        this.setState ({
            dialogOpen: props.open,
            spending: this.spending
        })
    }


    spending = {
        description: "",
        amountInCents: 0,
        fromGuid: null,
        toGuid: [],
        toAll: false,
        guid: ""
    }
    validate() {
        if (this.state.spending != null && this.state.spending.amountInCents != 0 && this.state.spending.fromGuid != null && (this.spending.toAll || this.spending.toGuid.length > 0))
            return true;
        return false;
    }

    render() {
        return (
            <span>
                <span onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.children}
                </span>
                <Dialog fullScreen={false} aria-labelledby="customized-dialog-title" open={this.state.dialogOpen} onClose={() => { this.setState({ dialogOpen: false }) }}>
                    <DialogTitle id="customized-dialog-title">{this.props.mode == 'edit' ? 'Edit' : 'Add'} Spending</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => { }}>
                            <FormGroup>
                                <TextField
                                    id="description"
                                    required
                                    label="Description"
                                    autoFocus
                                    defaultValue={this.spending.description}
                                    onChange={(e) => this.spending.description = event.target.value}
                                    margin="normal"
                                />
                                <TextField
                                    id="amount"
                                    required
                                    label="Amount"
                                    type="number"
                                    defaultValue={this.spending.amountInCents}
                                    onChange={(e) => this.spending.amountInCents = event.target.value}
                                    margin="normal"
                                />
                                <br/>
                                <InputLabel htmlFor="select-from">From</InputLabel>
                                <Select
                                input={<Input id="select-from" />}
                                    value={this.state.spending.fromGuid}
                                    onChange={(e) => { this.spending.fromGuid = e.target.value; this.setState({ spending: this.spending }) }}
                                >
                                    {
                                        this.tour.persons.map(p => (<MenuItem value={p.guid} key={p.guid}>{p.name}</MenuItem>))
                                    }
                                </Select>
                                <br/>

                                <InputLabel htmlFor="select-multiple-checkbox">To</InputLabel>
                                <Select
                                    multiple
                                    variant='outlined'
                                    style={{
                                        minWidth: 120,
                                        maxWidth: 600
                                    }
                                    }
                                    value={this.state.spending.toGuid}
                                    onChange={(e) => {
                                        //alert('options ' + JSON.stringify(e.target, null, 2))
                                        this.spending.toGuid = e.target.value;
                                        //alert('sp ' + JSON.stringify(this.spending, null, 2))
                                        this.setState({ spending: this.spending });
                                    }}
                                    input={<Input id="select-multiple-checkbox" />}
                                    renderValue={selected => selected.map(gg => this.tour.persons.find(pp => pp.guid === gg) == null
                                        ? '$$-' + gg + '-$$'
                                        : this.tour.persons.find(pp => pp.guid === gg).name).join(', ')}
                                    MenuProps={MenuProps}
                                >
                                    {this.tour.persons.map(p => (
                                        <MenuItem key={p.guid} value={p.guid}>
                                            <Checkbox checked={this.state.spending.toGuid.indexOf(p.guid) > -1} />
                                            <ListItemText primary={p.name} />
                                        </MenuItem>
                                    ))}
                                </Select>
                                <FormControlLabel
                                    control={
                                        <Switch
                                            value={true}
                                            checked={this.state.spending.toAll}
                                            onChange={(e) => {
                                                this.spending.toAll = e.target.checked;
                                                this.setState({ spending: this.spending });
                                            }}
                                         
                                        />
                                    }
                                    label="To All"
                                />
                                <TextField
                                    id="type"
                                    label="Spending Type"
                                    defaultValue={this.spending.type}
                                    onChange={(e) => this.spending.type = event.target.value}
                                    margin="normal"
                                />
                                {/*<Autocomplete
                                    suggestions={this.tour.spendings.map((s) => s.type)
                                        .filter(s => s) // not empty or null
                                        .filter((value, index, self) => self.indexOf(value) === index) // remove dups
                                    }
                                    hint="Spending Type"
                                    defaultValue={this.spending.type}
                                    onFill={(val) => { this.spending.type = val; }}
                                />*/}
                          </FormGroup>
                            <br  />
                        </form>
                    </DialogContent>
                    <DialogActions>
                        <Button color="primary" size='large' variant='outlined' onClick={() => {
                            //alert('sp: ' + JSON.stringify(this.spending, null, 2))
                            if (this.validate()) {
                                (this.props.mode === "add"
                                    ? AppState.addSpending(this.props.app, this.tour.id, this.spending)
                                    : AppState.editSpending(this.props.app, this.tour.id, this.spending)
                                )
                                    .then(this.setState({ dialogOpen: false }))
                                    .then(() => { AppState.loadTour(this.props.app, this.tour.id) })
                            } else {
                                alert("not all info is entered correctly");
                            }
                              
                        }}>{this.props.actionButtonText}</Button>
                        <Button
                            color="secondary" size='large' variant='outlined' 
                            onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</Button>
                    </DialogActions>
                </Dialog>
            </span>
        )
    }
}